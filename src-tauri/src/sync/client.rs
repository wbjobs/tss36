use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, Utc};
use parking_lot::RwLock;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::protocol::*;
use super::server::{IndexServer, ServerController};
use super::version_graph::{ConflictInfo, VersionGraphStore, VersionNode};
use crate::db;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStats {
    pub files_synced: u64,
    pub blocks_transferred: u64,
    pub bytes_transferred: u64,
    pub conflicts_detected: u64,
    pub conflicts_resolved: u64,
    pub last_sync_time: Option<DateTime<Local>>,
    pub connected_clients: u64,
    pub is_server_mode: bool,
}

impl Default for SyncStats {
    fn default() -> Self {
        Self {
            files_synced: 0,
            blocks_transferred: 0,
            bytes_transferred: 0,
            conflicts_detected: 0,
            conflicts_resolved: 0,
            last_sync_time: None,
            connected_clients: 0,
            is_server_mode: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub versions_pulled: u64,
    pub blocks_pulled: u64,
    pub bytes_pulled: u64,
    pub conflicts_found: u64,
}

pub struct SyncClient {
    pub client_id: String,
    pub hostname: String,
    pub server_addr: Arc<RwLock<Option<String>>>,
    pub is_connected: Arc<AtomicBool>,
    pub is_syncing: Arc<AtomicBool>,
    pub sync_stats: Arc<RwLock<SyncStats>>,
    pub conflicts: Arc<RwLock<Vec<ConflictInfo>>>,
    pub version_graph_store: Arc<RwLock<VersionGraphStore>>,
    pub known_peers: Arc<RwLock<Vec<PeerInfo>>>,
    pub server_controller: Arc<RwLock<Option<ServerController>>>,
    pub client_join_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    pub pending_blocks: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    pub p2p_port: Arc<RwLock<Option<u16>>>,
    pub watch_path: Arc<RwLock<String>>,
}

impl SyncClient {
    pub fn new(hostname: String) -> Self {
        Self {
            client_id: Uuid::new_v4().to_string(),
            hostname,
            server_addr: Arc::new(RwLock::new(None)),
            is_connected: Arc::new(AtomicBool::new(false)),
            is_syncing: Arc::new(AtomicBool::new(false)),
            sync_stats: Arc::new(RwLock::new(SyncStats::default())),
            conflicts: Arc::new(RwLock::new(Vec::new())),
            version_graph_store: Arc::new(RwLock::new(VersionGraphStore::new())),
            known_peers: Arc::new(RwLock::new(Vec::new())),
            server_controller: Arc::new(RwLock::new(None)),
            client_join_handle: Arc::new(RwLock::new(None)),
            pending_blocks: Arc::new(RwLock::new(HashMap::new())),
            p2p_port: Arc::new(RwLock::new(None)),
            watch_path: Arc::new(RwLock::new(String::new())),
        }
    }

    pub async fn start_server(&self, port: u16) -> Result<String> {
        let mut server = IndexServer::new();
        let addr = format!("0.0.0.0:{}", port);
        let (bound_addr, controller) = server.start(&addr).await?;
        info!("索引服务器启动于 {}", bound_addr);

        *self.server_addr.write() = Some(bound_addr.to_string());
        self.sync_stats.write().is_server_mode = true;
        *self.server_controller.write() = Some(controller);

        self.connect_to_server(&bound_addr.to_string()).await?;

        Ok(bound_addr.to_string())
    }

    pub async fn stop_server(&self) -> Result<()> {
        self.disconnect().await?;
        if let Some(mut ctrl) = self.server_controller.write().take() {
            ctrl.stop();
        }
        self.sync_stats.write().is_server_mode = false;
        *self.server_addr.write() = None;
        Ok(())
    }

    pub async fn connect_to_server(&self, addr: &str) -> Result<()> {
        let parsed_addr: SocketAddr = addr.parse().context("无效的服务器地址")?;
        let stream = TcpStream::connect(parsed_addr)
            .await
            .with_context(|| format!("连接服务器 {} 失败", addr))?;

        *self.server_addr.write() = Some(addr.to_string());

        let client_id = self.client_id.clone();
        let hostname = self.hostname.clone();
        let watch_path = self.watch_path.read().clone();
        let p2p_port = self.p2p_port.read().unwrap_or(0);

        let _ = self.start_p2p_listener().await?;
        let actual_p2p_port = self.p2p_port.read().unwrap_or(0);

        let is_connected = self.is_connected.clone();
        let known_peers = self.known_peers.clone();
        let version_graph_store = self.version_graph_store.clone();
        let sync_stats = self.sync_stats.clone();

        let handle = tokio::spawn(async move {
            let mut stream = stream;
            let reg = RegisterClient {
                client_id: client_id.clone(),
                hostname: hostname.clone(),
                listen_port: actual_p2p_port,
                watched_path: watch_path,
            };
            let env = Envelope {
                msg_type: MessageType::RegisterClient,
                payload: match serialize_msg(&reg) {
                    Ok(d) => d,
                    Err(e) => { error!("序列化失败: {}", e); return; }
                },
            };
            if write_envelope(&mut stream, &env).await.is_err() {
                return;
            }
            is_connected.store(true, Ordering::Relaxed);

            let mut heartbeat_count = 0u64;
            loop {
                if !is_connected.load(Ordering::Relaxed) {
                    break;
                }
                tokio::select! {
                    r = read_envelope(&mut stream) => {
                        match r {
                            Ok(env) => {
                                handle_server_message(
                                    env,
                                    &version_graph_store,
                                    &known_peers,
                                    &sync_stats,
                                );
                            }
                            Err(e) => {
                                debug!("与服务器断开: {}", e);
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                        heartbeat_count += 1;
                        let hb = Heartbeat {
                            client_id: client_id.clone(),
                            timestamp: Utc::now(),
                        };
                        if let Ok(data) = serialize_msg(&hb) {
                            let env = Envelope {
                                msg_type: MessageType::Heartbeat,
                                payload: data,
                            };
                            if write_envelope(&mut stream, &env).await.is_err() {
                                break;
                            }
                        }
                        let _ = heartbeat_count;
                    }
                }
            }
            is_connected.store(false, Ordering::Relaxed);
        });

        *self.client_join_handle.write() = Some(handle);
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.is_connected.store(false, Ordering::Relaxed);
        if let Some(handle) = self.client_join_handle.write().take() {
            handle.abort();
        }
        Ok(())
    }

    pub async fn publish_local_versions(&self, conn: &Connection) -> Result<u64> {
        let server_addr = self
            .server_addr
            .read()
            .clone()
            .ok_or_else(|| anyhow!("未连接到服务器"))?;
        let addr: SocketAddr = server_addr.parse()?;
        let mut stream = TcpStream::connect(addr).await?;

        let reg = RegisterClient {
            client_id: self.client_id.clone(),
            hostname: self.hostname.clone(),
            listen_port: self.p2p_port.read().unwrap_or(0),
            watched_path: self.watch_path.read().clone(),
        };
        let env = Envelope {
            msg_type: MessageType::RegisterClient,
            payload: serialize_msg(&reg)?,
        };
        write_envelope(&mut stream, &env).await?;
        let _resp = read_envelope(&mut stream).await?;

        let mut files_count = 0u64;
        let mut stmt = conn.prepare(
            "SELECT f.id, f.path, f.file_type FROM files f"
        )?;
        let files: Vec<(i64, String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();

        for (file_id, file_path, _file_type) in files {
            let versions = db::get_file_versions(conn, file_id)?;
            for v in versions {
                let block_entries = db::get_version_block_hashes(conn, v.id)?;
                let block_hashes: Vec<String> = block_entries
                    .into_iter()
                    .map(|(_, h)| h)
                    .collect();
                let ts: DateTime<Utc> = DateTime::from_naive_utc_and_offset(
                    v.timestamp.naive_utc(),
                    chrono::Utc,
                );
                let publish = PublishVersion {
                    client_id: self.client_id.clone(),
                    file_path: file_path.clone(),
                    version_number: v.version_number,
                    content_hash: v.new_content_hash.clone(),
                    block_hashes,
                    timestamp: ts,
                    file_size: v.file_size,
                    parent_hash: if v.version_number > 1 {
                        Some(v.prev_content_hash.clone())
                    } else {
                        None
                    },
                };
                let env = Envelope {
                    msg_type: MessageType::PublishVersion,
                    payload: serialize_msg(&publish)?,
                };
                write_envelope(&mut stream, &env).await?;
                let _ack = read_envelope(&mut stream).await?;
                files_count += 1;
            }
        }

        Ok(files_count)
    }

    pub async fn pull_remote_versions(&self, conn: &Connection) -> Result<SyncResult> {
        if !self.is_connected.load(Ordering::Relaxed) {
            anyhow::bail!("未连接到服务器");
        }
        self.is_syncing.store(true, Ordering::Relaxed);

        let server_addr = self
            .server_addr
            .read()
            .clone()
            .ok_or_else(|| anyhow!("未连接"))?;
        let addr: SocketAddr = server_addr.parse()?;
        let mut stream = TcpStream::connect(addr).await?;

        let reg = RegisterClient {
            client_id: self.client_id.clone(),
            hostname: self.hostname.clone(),
            listen_port: self.p2p_port.read().unwrap_or(0),
            watched_path: self.watch_path.read().clone(),
        };
        let env = Envelope {
            msg_type: MessageType::RegisterClient,
            payload: serialize_msg(&reg)?,
        };
        write_envelope(&mut stream, &env).await?;
        let _resp = read_envelope(&mut stream).await?;

        let mut versions_pulled = 0u64;
        let mut blocks_pulled = 0u64;
        let mut bytes_pulled = 0u64;
        let mut conflicts_found = 0u64;

        let local_files: Vec<(i64, String, String)> = {
            let mut stmt = conn.prepare("SELECT id, path, file_type FROM files")?;
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .filter_map(|r| r.ok())
                .collect()
        };

        for (file_id, file_path, file_type) in local_files {
            let req = RequestVersionGraph {
                file_path: file_path.clone(),
            };
            let env = Envelope {
                msg_type: MessageType::RequestVersionGraph,
                payload: serialize_msg(&req)?,
            };
            write_envelope(&mut stream, &env).await?;
            let env = read_envelope(&mut stream).await?;
            let graph_resp: VersionGraphResponse = deserialize_msg(&env.payload)?;

            let mut vg = VersionGraphStore::new();
            for node in graph_resp.nodes {
                let vn = VersionNode::new(
                    &node.client_id,
                    node.version_number,
                    &node.content_hash,
                    node.timestamp,
                    node.file_size,
                    node.parent_hash,
                    node.block_hashes,
                );
                let conflicts = vg.publish_version(&file_path, vn);
                conflicts_found += conflicts.len() as u64;
                if !conflicts.is_empty() {
                    self.conflicts.write().extend(conflicts);
                }
            }

            let local_block_hashes: HashSet<String> = {
                let mut set = HashSet::new();
                let versions = db::get_file_versions(conn, file_id)?;
                for v in versions {
                    let entries = db::get_version_block_hashes(conn, v.id)?;
                    for (_, h) in entries {
                        set.insert(h);
                    }
                }
                set
            };

            let remote_graph = vg.graphs.get(&file_path);
            if let Some(g) = remote_graph {
                let missing = g.get_missing_blocks(&local_block_hashes);
                if !missing.is_empty() {
                    let pull_req = PullMissingBlocks {
                        client_id: self.client_id.clone(),
                        missing_hashes: missing.clone(),
                    };
                    let env = Envelope {
                        msg_type: MessageType::PullMissingBlocks,
                        payload: serialize_msg(&pull_req)?,
                    };
                    write_envelope(&mut stream, &env).await?;
                    let env = read_envelope(&mut stream).await?;
                    let pull_resp: PullMissingBlocksResponse = deserialize_msg(&env.payload)?;

                    for br in &pull_resp.blocks {
                        if br.found && !br.data.is_empty() {
                            db::insert_or_get_block(conn, &br.block_hash, &br.data)?;
                            blocks_pulled += 1;
                            bytes_pulled += br.data.len() as u64;
                        }
                    }
                }

                if let Some(latest) = g.resolve_conflict_lww() {
                    if latest.client_id != self.client_id {
                        let prev_vn = db::get_next_version_number(conn, file_id)?;
                        let prev_hash: String = db::get_file_by_path(conn, &file_path)?
                            .map(|f| f.current_hash)
                            .unwrap_or_default();
                        let _ = (prev_vn, prev_hash);
                        versions_pulled += 1;
                    }
                }
            }
        }

        {
            let mut stats = self.sync_stats.write();
            stats.files_synced += local_files.len() as u64;
            stats.blocks_transferred += blocks_pulled;
            stats.bytes_transferred += bytes_pulled;
            stats.conflicts_detected += conflicts_found;
            stats.last_sync_time = Some(Local::now());
        }

        self.is_syncing.store(false, Ordering::Relaxed);
        Ok(SyncResult {
            versions_pulled,
            blocks_pulled,
            bytes_pulled,
            conflicts_found,
        })
    }

    pub async fn start_p2p_listener(&self) -> Result<u16> {
        if let Some(p) = *self.p2p_port.read() {
            return Ok(p);
        }
        let listener = TcpListener::bind("0.0.0.0:0").await?;
        let port = listener.local_addr()?.port();
        *self.p2p_port.write() = Some(port);

        let pending_blocks = self.pending_blocks.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => {
                        let pending_blocks = pending_blocks.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_p2p_stream(stream, pending_blocks).await {
                                warn!("P2P 错误: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("P2P accept 错误: {}", e);
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }
                }
            }
        });
        Ok(port)
    }

    pub fn get_sync_stats(&self) -> SyncStats {
        self.sync_stats.read().clone()
    }

    pub fn get_conflicts(&self) -> Vec<ConflictInfo> {
        self.conflicts.read().clone()
    }

    pub async fn fetch_block_from_peer(
        &self,
        peer: &PeerInfo,
        block_hash: &str,
    ) -> Result<Option<Vec<u8>>> {
        let addr = format!("{}:{}", peer.address, peer.listen_port);
        let addr: SocketAddr = addr.parse()?;
        let mut stream = TcpStream::connect(addr).await?;
        let req = RequestBlock {
            block_hash: block_hash.to_string(),
        };
        let env = Envelope {
            msg_type: MessageType::RequestBlock,
            payload: serialize_msg(&req)?,
        };
        write_envelope(&mut stream, &env).await?;
        let env = read_envelope(&mut stream).await?;
        let resp: BlockResponse = deserialize_msg(&env.payload)?;
        if resp.found {
            Ok(Some(resp.data))
        } else {
            Ok(None)
        }
    }

    pub fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.known_peers.read().clone()
    }

    pub fn resolve_conflict_locally(
        &self,
        file_path: &str,
        choose_local: bool,
    ) -> Result<()> {
        let mut conflicts = self.conflicts.write();
        if choose_local {
            conflicts.retain(|c| c.file_path != file_path);
        } else {
            conflicts.retain(|c| c.file_path != file_path);
        }
        self.sync_stats.write().conflicts_resolved += 1;
        Ok(())
    }
}

async fn handle_p2p_stream(
    mut stream: TcpStream,
    pending_blocks: Arc<RwLock<HashMap<String, Vec<u8>>>>,
) -> Result<()> {
    loop {
        let env = read_envelope(&mut stream).await?;
        if env.msg_type == MessageType::RequestBlock {
            let req: RequestBlock = deserialize_msg(&env.payload)?;
            let data = pending_blocks.read().get(&req.block_hash).cloned();
            let resp = BlockResponse {
                block_hash: req.block_hash,
                data: data.clone().unwrap_or_default(),
                found: data.is_some(),
            };
            let env = Envelope {
                msg_type: MessageType::BlockResponse,
                payload: serialize_msg(&resp)?,
            };
            write_envelope(&mut stream, &env).await?;
        } else {
            break;
        }
    }
    Ok(())
}

fn handle_server_message(
    env: Envelope,
    _graphs: &Arc<RwLock<VersionGraphStore>>,
    known_peers: &Arc<RwLock<Vec<PeerInfo>>>,
    stats: &Arc<RwLock<SyncStats>>,
) {
    match env.msg_type {
        MessageType::PeerList => {
            if let Ok(list) = deserialize_msg::<PeerList>(&env.payload) {
                *known_peers.write() = list.peers;
                stats.write().connected_clients = list.peers.len() as u64;
            }
        }
        MessageType::ConflictAlert => {
            if let Ok(_alert) = deserialize_msg::<ConflictAlert>(&env.payload) {
                stats.write().conflicts_detected += 1;
            }
        }
        MessageType::VersionGraphResponse => {}
        MessageType::VersionPublished => {}
        _ => {}
    }
}
