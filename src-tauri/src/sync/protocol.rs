use anyhow::{Context, Result};
use bytes::{Buf, BufMut, BytesMut};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    RegisterClient = 1,
    ClientRegistered = 2,
    PublishVersion = 3,
    VersionPublished = 4,
    RequestVersionGraph = 5,
    VersionGraphResponse = 6,
    RequestBlock = 7,
    BlockResponse = 8,
    RequestPeers = 9,
    PeerList = 10,
    ConflictAlert = 11,
    SyncStatus = 12,
    Heartbeat = 13,
    PullMissingBlocks = 14,
    PullMissingBlocksResponse = 15,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub msg_type: MessageType,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterClient {
    pub client_id: String,
    pub hostname: String,
    pub listen_port: u16,
    pub watched_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRegistered {
    pub client_id: String,
    pub assigned_id: u64,
    pub server_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishVersion {
    pub client_id: String,
    pub file_path: String,
    pub version_number: i64,
    pub content_hash: String,
    pub block_hashes: Vec<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub file_size: i64,
    pub parent_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionPublished {
    pub success: bool,
    pub conflict_detected: bool,
    pub conflicting_client: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestVersionGraph {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteVersionNode {
    pub client_id: String,
    pub version_number: i64,
    pub content_hash: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub file_size: i64,
    pub parent_hash: Option<String>,
    pub block_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionGraphResponse {
    pub file_path: String,
    pub nodes: Vec<RemoteVersionNode>,
    pub has_conflict: bool,
    pub latest_client_id: String,
    pub latest_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBlock {
    pub block_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResponse {
    pub block_hash: String,
    pub data: Vec<u8>,
    pub found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestPeers {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub client_id: String,
    pub hostname: String,
    pub address: String,
    pub listen_port: u16,
    pub version_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerList {
    pub peers: Vec<PeerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictAlert {
    pub file_path: String,
    pub local_version: i64,
    pub local_hash: String,
    pub local_timestamp: chrono::DateTime<chrono::Utc>,
    pub remote_client_id: String,
    pub remote_version: i64,
    pub remote_hash: String,
    pub remote_timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub client_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullMissingBlocks {
    pub client_id: String,
    pub missing_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullMissingBlocksResponse {
    pub blocks: Vec<BlockResponse>,
    pub remaining: usize,
}

pub fn serialize_msg<T: Serialize>(msg: &T) -> Result<Vec<u8>> {
    bincode::serialize(msg).context("bincode 序列化失败")
}

pub fn deserialize_msg<T: DeserializeOwned>(data: &[u8]) -> Result<T> {
    bincode::deserialize(data).context("bincode 反序列化失败")
}

pub async fn write_frame_raw(stream: &mut TcpStream, data: &[u8]) -> Result<()> {
    let len = data.len() as u32;
    let mut buf = BytesMut::with_capacity(4 + data.len());
    buf.put_u32(len);
    buf.extend_from_slice(data);
    stream
        .write_all(&buf)
        .await
        .context("写入 TCP 帧失败")?;
    stream.flush().await.context("TCP flush 失败")?;
    Ok(())
}

pub async fn read_frame_raw(stream: &mut TcpStream) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .context("读取帧长度头失败")?;
    let len = (&len_buf[..]).get_u32() as usize;
    if len > 256 * 1024 * 1024 {
        anyhow::bail!("帧过大: {} bytes", len);
    }
    let mut payload = vec![0u8; len];
    stream
        .read_exact(&mut payload)
        .await
        .context("读取帧数据失败")?;
    Ok(payload)
}

pub async fn write_envelope(stream: &mut TcpStream, env: &Envelope) -> Result<()> {
    let data = serialize_msg(env)?;
    write_frame_raw(stream, &data).await
}

pub async fn read_envelope(stream: &mut TcpStream) -> Result<Envelope> {
    let data = read_frame_raw(stream).await?;
    deserialize_msg(&data)
}
