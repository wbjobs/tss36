use serde::{Deserialize, Serialize};
use tauri::State;
use crate::state::AppState;
use crate::sync::{ConflictInfo, PeerInfo, SyncResult, SyncStats};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncStatsDto {
    files_synced: u64,
    blocks_transferred: u64,
    bytes_transferred: u64,
    conflicts_detected: u64,
    conflicts_resolved: u64,
    last_sync_time: Option<String>,
    connected_clients: u64,
    is_server_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConflictInfoDto {
    file_path: String,
    local_node_id: String,
    remote_node_id: String,
    local_client_id: String,
    remote_client_id: String,
    local_timestamp: String,
    remote_timestamp: String,
    resolved_winner: String,
    resolution_strategy: String,
    needs_manual_merge: bool,
}

impl From<ConflictInfo> for ConflictInfoDto {
    fn from(c: ConflictInfo) -> Self {
        Self {
            file_path: c.file_path,
            local_node_id: c.local_node_id,
            remote_node_id: c.remote_node_id,
            local_client_id: c.local_client_id,
            remote_client_id: c.remote_client_id,
            local_timestamp: c.local_timestamp.to_rfc3339(),
            remote_timestamp: c.remote_timestamp.to_rfc3339(),
            resolved_winner: c.resolved_winner,
            resolution_strategy: c.resolution_strategy,
            needs_manual_merge: c.needs_manual_merge,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PeerInfoDto {
    client_id: String,
    hostname: String,
    address: String,
    listen_port: u16,
    version_count: i64,
}

impl From<PeerInfo> for PeerInfoDto {
    fn from(p: PeerInfo) -> Self {
        Self {
            client_id: p.client_id,
            hostname: p.hostname,
            address: p.address,
            listen_port: p.listen_port,
            version_count: p.version_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncResultDto {
    versions_pulled: u64,
    blocks_pulled: u64,
    bytes_pulled: u64,
    conflicts_found: u64,
}

impl From<SyncResult> for SyncResultDto {
    fn from(r: SyncResult) -> Self {
        Self {
            versions_pulled: r.versions_pulled,
            blocks_pulled: r.blocks_pulled,
            bytes_pulled: r.bytes_pulled,
            conflicts_found: r.conflicts_found,
        }
    }
}

#[tauri::command]
pub async fn start_sync_server(
    port: u16,
    state: State<'_, AppState>,
) -> Result<String, String> {
    state
        .sync_client
        .start_server(port)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_sync_server(state: State<'_, AppState>) -> Result<(), String> {
    state
        .sync_client
        .stop_server()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn connect_to_server(
    address: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .sync_client
        .connect_to_server(&address)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disconnect_sync(state: State<'_, AppState>) -> Result<(), String> {
    state
        .sync_client
        .disconnect()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn publish_local_versions(
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let conn_arc = state.db_connection.clone();
    let sync_client = state.sync_client.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let conn = conn_arc.read();
        // sync_client.publish_local_versions 需要 &Connection，
        // 但它是 async，所以我们在 spawn_blocking 中直接返回 conn，让上层处理
        // 实际上我们只能在 sync client 内部完成工作，这里用更简单的模式：
        drop(conn);
        Ok::<u64, String>(0)
    });
    // 简化：由于 publish 需要 async + db 同步借用冲突，这里直接返回 0
    // 实际应用中可以把 DB 操作移到 sync_client 内部用 channels 处理
    let _ = handle.await.map_err(|e| e.to_string())?;

    // 这里为了编译通过，使用更简单的实现：
    // 在真正实现中，建议使用单独的 DB task + message queue 处理同步 I/O
    let conn = state.db_connection.clone();
    let client = state.sync_client.clone();
    match client.publish_local_versions(&conn.read()).await {
        Ok(n) => Ok(n),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn pull_remote_versions(
    state: State<'_, AppState>,
) -> Result<SyncResultDto, String> {
    let conn = state.db_connection.clone();
    let client = state.sync_client.clone();
    match client.pull_remote_versions(&conn.read()).await {
        Ok(r) => Ok(SyncResultDto::from(r)),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_sync_stats(state: State<'_, AppState>) -> SyncStatsDto {
    let stats: SyncStats = state.sync_client.get_sync_stats();
    SyncStatsDto {
        files_synced: stats.files_synced,
        blocks_transferred: stats.blocks_transferred,
        bytes_transferred: stats.bytes_transferred,
        conflicts_detected: stats.conflicts_detected,
        conflicts_resolved: stats.conflicts_resolved,
        last_sync_time: stats.last_sync_time.map(|t| t.to_rfc3339()),
        connected_clients: stats.connected_clients,
        is_server_mode: stats.is_server_mode,
    }
}

#[tauri::command]
pub fn get_conflicts(state: State<'_, AppState>) -> Vec<ConflictInfoDto> {
    state
        .sync_client
        .get_conflicts()
        .into_iter()
        .map(ConflictInfoDto::from)
        .collect()
}

#[tauri::command]
pub fn resolve_conflict(
    file_path: String,
    choose_local: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .sync_client
        .resolve_conflict_locally(&file_path, choose_local)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_connected_peers(state: State<'_, AppState>) -> Vec<PeerInfoDto> {
    state
        .sync_client
        .get_connected_peers()
        .into_iter()
        .map(PeerInfoDto::from)
        .collect()
}
