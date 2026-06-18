use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info, warn};

use super::protocol::*;
use super::version_graph::{VersionGraphStore, VersionNode};

#[derive(Debug, Clone)]
pub struct ClientEntry {
    pub client_id: String,
    pub hostname: String,
    pub address: SocketAddr,
    pub listen_port: u16,
    pub watched_path: String,
    pub last_heartbeat: DateTime<Utc>,
    pub file_count: u64,
    pub version_count: u64,
}

pub struct IndexServer {
    pub clients: Arc<std::sync::Mutex<HashMap<String, ClientEntry>>>,
    pub version_graphs: Arc<RwLock<VersionGraphStore>>,
    pub block_index: Arc<std::sync::Mutex<HashMap<String, HashSet<String>>>>,
    pub file_owners: Arc<std::sync::Mutex<HashMap<String, HashSet<String>>>>,
    pub shutdown: Arc<AtomicBool>,
}

impl IndexServer {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(std::sync::Mutex::new(HashMap::new())),
            version_graphs: Arc::new(RwLock::new(VersionGraphStore::new())),
            block_index: Arc::new(std::sync::Mutex::new(HashMap::new())),
            file_owners: Arc::new(std::sync::Mutex::new(HashMap::new())),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(
        &mut self,
        addr: &str,
    ) -> Result<(SocketAddr, ServerController)> {
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("绑定监听地址 {} 失败", addr))?;
        let bound_addr = listener.local_addr()?;

        let clients = self.clients.clone();
        let graphs = self.version_graphs.clone();
        let block_index = self.block_index.clone();
        let file_owners = self.file_owners.clone();
        let shutdown = self.shutdown.clone();

        let join_handle = tokio::spawn(async move {
            loop {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                let accept = tokio::select! {
                    r = listener.accept() => r,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => continue,
                };
                match accept {
                    Ok((stream, addr)) => {
                        let clients = clients.clone();
                        let graphs = graphs.clone();
                        let block_index = block_index.clone();
                        let file_owners = file_owners.clone();
                        let shutdown = shutdown.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client(
                                stream, addr, clients, graphs, block_index, file_owners, shutdown,
                            ).await {
                                error!("处理客户端错误: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Accept 错误: {}", e);
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
            info!("索引服务器已停止");
        });

        let controller = ServerController {
            shutdown: self.shutdown.clone(),
            join_handle: Some(join_handle),
            address: bound_addr,
        };
        Ok((bound_addr, controller))
    }

    pub fn stop(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

pub struct ServerController {
    pub shutdown: Arc<AtomicBool>,
    pub join_handle: Option<tokio::task::JoinHandle<()>>,
    pub address: SocketAddr,
}

impl ServerController {
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

async fn handle_client(
    mut stream: TcpStream,
    addr: SocketAddr,
    clients: Arc<std::sync::Mutex<HashMap<String, ClientEntry>>>,
    graphs: Arc<RwLock<VersionGraphStore>>,
    block_index: Arc<std::sync::Mutex<HashMap<String, HashSet<String>>>>,
    file_owners: Arc<std::sync::Mutex<HashMap<String, HashSet<String>>>>,
    shutdown: Arc<AtomicBool>,
) -> Result<()> {
    let mut registered_client_id: Option<String> = None;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let envelope = tokio::select! {
            r = read_envelope(&mut stream) => r,
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                continue;
            }
        };

        let env = match envelope {
            Ok(e) => e,
            Err(e) => {
                debug!("客户端 {} 断开: {}", addr, e);
                break;
            }
        };

        match env.msg_type {
            MessageType::RegisterClient => {
                let msg: RegisterClient = deserialize_msg(&env.payload)?;
                let cid = msg.client_id.clone();
                let entry = ClientEntry {
                    client_id: cid.clone(),
                    hostname: msg.hostname,
                    address: addr,
                    listen_port: msg.listen_port,
                    watched_path: msg.watched_path,
                    last_heartbeat: Utc::now(),
                    file_count: 0,
                    version_count: 0,
                };
                clients
                    .lock()
                    .unwrap()
                    .insert(cid.clone(), entry);
                let resp = ClientRegistered {
                    client_id: cid.clone(),
                    assigned_id: 1,
                    server_version: "0.1.0".to_string(),
                };
                registered_client_id = Some(cid);
                send_response(&mut stream, MessageType::ClientRegistered, &resp).await?;
            }

            MessageType::PublishVersion => {
                let msg: PublishVersion = deserialize_msg(&env.payload)?;
                let mut conflict_detected = false;
                let mut conflicting_client = None;

                let node = VersionNode::new(
                    &msg.client_id,
                    msg.version_number,
                    &msg.content_hash,
                    msg.timestamp,
                    msg.file_size,
                    msg.parent_hash,
                    msg.block_hashes.clone(),
                );

                let conflicts = {
                    let mut store = graphs.write();
                    store.publish_version(&msg.file_path, node)
                };

                if !conflicts.is_empty() {
                    conflict_detected = true;
                    conflicting_client = conflicts
                        .first()
                        .map(|c| c.remote_client_id.clone());
                }

                {
                    let mut bi = block_index.lock().unwrap();
                    for bh in &msg.block_hashes {
                        bi.entry(bh.clone())
                            .or_insert_with(HashSet::new)
                            .insert(msg.client_id.clone());
                    }
                }

                {
                    let mut fo = file_owners.lock().unwrap();
                    fo.entry(msg.file_path)
                        .or_insert_with(HashSet::new)
                        .insert(msg.client_id.clone());
                }

                if let Some(cid) = registered_client_id.as_ref() {
                    if let Some(entry) = clients.lock().unwrap().get_mut(cid) {
                        entry.version_count += 1;
                    }
                }

                let resp = VersionPublished {
                    success: true,
                    conflict_detected,
                    conflicting_client,
                };
                send_response(&mut stream, MessageType::VersionPublished, &resp).await?;
            }

            MessageType::RequestVersionGraph => {
                let msg: RequestVersionGraph = deserialize_msg(&env.payload)?;
                let store = graphs.read();
                let (nodes_vec, has_conflict, latest_cid, latest_ts) = match store.graphs.get(&msg.file_path) {
                    Some(g) => {
                        let nodes_vec: Vec<RemoteVersionNode> = g
                            .nodes
                            .values()
                            .map(|n| RemoteVersionNode {
                                client_id: n.client_id.clone(),
                                version_number: n.version_number,
                                content_hash: n.content_hash.clone(),
                                timestamp: n.timestamp,
                                file_size: n.file_size,
                                parent_hash: n.parent_hash.clone(),
                                block_hashes: n.block_hashes.clone(),
                            })
                            .collect();
                        let has_conflict = g.heads.len() > 1;
                        let (latest_cid, latest_ts) = g
                            .resolve_conflict_lww()
                            .map(|n| (n.client_id.clone(), n.timestamp))
                            .unwrap_or_else(|| (String::new(), Utc::now()));
                        (nodes_vec, has_conflict, latest_cid, latest_ts)
                    }
                    None => (Vec::new(), false, String::new(), Utc::now()),
                };

                let resp = VersionGraphResponse {
                    file_path: msg.file_path,
                    nodes: nodes_vec,
                    has_conflict,
                    latest_client_id: latest_cid,
                    latest_timestamp: latest_ts,
                };
                send_response(&mut stream, MessageType::VersionGraphResponse, &resp).await?;
            }

            MessageType::RequestPeers => {
                let msg: RequestPeers = deserialize_msg(&env.payload)?;
                let mut peer_list = Vec::new();
                let owner_ids: HashSet<String> = file_owners
                    .lock()
                    .unwrap()
                    .get(&msg.file_path)
                    .cloned()
                    .unwrap_or_default();
                let all_clients = clients.lock().unwrap().clone();
                for cid in owner_ids {
                    if let Some(entry) = all_clients.get(&cid) {
                        peer_list.push(PeerInfo {
                            client_id: entry.client_id.clone(),
                            hostname: entry.hostname.clone(),
                            address: entry.address.ip().to_string(),
                            listen_port: entry.listen_port,
                            version_count: entry.version_count as i64,
                        });
                    }
                }
                let resp = PeerList { peers: peer_list };
                send_response(&mut stream, MessageType::PeerList, &resp).await?;
            }

            MessageType::Heartbeat => {
                let msg: Heartbeat = deserialize_msg(&env.payload)?;
                if let Some(entry) = clients.lock().unwrap().get_mut(&msg.client_id) {
                    entry.last_heartbeat = msg.timestamp;
                }
            }

            MessageType::PullMissingBlocks => {
                let msg: PullMissingBlocks = deserialize_msg(&env.payload)?;
                let mut blocks = Vec::new();
                let mut queue = Vec::new();
                {
                    let bi = block_index.lock().unwrap();
                    for h in &msg.missing_hashes {
                        if let Some(owners) = bi.get(h) {
                            if let Some(owner_cid) = owners.iter().next() {
                                let clients_map = clients.lock().unwrap();
                                if let Some(entry) = clients_map.get(owner_cid) {
                                    queue.push((h.clone(), *entry));
                                }
                            }
                        }
                    }
                }
                let blocks_map = Arc::new(std::sync::Mutex::new(HashMap::new()));
                let _ = blocks_map;
                for (hash, _entry) in queue {
                    blocks.push(BlockResponse {
                        block_hash: hash.clone(),
                        data: Vec::new(),
                        found: false,
                    });
                }
                let remaining = msg.missing_hashes.len().saturating_sub(blocks.len());
                let resp = PullMissingBlocksResponse { blocks, remaining };
                send_response(&mut stream, MessageType::PullMissingBlocksResponse, &resp).await?;
            }

            _ => {
                warn!("未处理的消息类型: {:?}", env.msg_type);
            }
        }
    }

    if let Some(cid) = registered_client_id {
        clients.lock().unwrap().remove(&cid);
        info!("客户端 {} 注销", cid);
    }
    Ok(())
}

async fn send_response<T: serde::Serialize>(
    stream: &mut TcpStream,
    msg_type: MessageType,
    msg: &T,
) -> Result<()> {
    let env = Envelope {
        msg_type,
        payload: serialize_msg(msg)?,
    };
    write_envelope(stream, &env).await
}
