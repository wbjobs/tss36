use std::sync::Arc;
use anyhow::{Context, Result};
use parking_lot::RwLock;
use rusqlite::Connection;
use crate::sync::SyncClient;
use crate::watcher::WatcherController;

pub struct AppState {
    pub db_connection: Arc<RwLock<Connection>>,
    pub watcher_handle: RwLock<Option<WatcherController>>,
    pub watched_path: RwLock<Option<String>>,
    pub sync_client: Arc<SyncClient>,
}

impl AppState {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("打开 SQLite 数据库失败: {}", db_path))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("设置 WAL 模式失败")?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .context("启用外键失败")?;
        let conn_arc = Arc::new(RwLock::new(conn));

        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown-host".to_string());

        Ok(Self {
            db_connection: conn_arc,
            watcher_handle: RwLock::new(None),
            watched_path: RwLock::new(None),
            sync_client: Arc::new(SyncClient::new(hostname)),
        })
    }
}

