use std::sync::Mutex;
use rusqlite::Connection;
use crate::watcher::WatcherController;

pub struct AppState {
    pub db_connection: Mutex<Connection>,
    pub watcher_handle: Mutex<Option<WatcherController>>,
    pub watched_path: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(db_path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(db_path)?;
        Ok(Self {
            db_connection: Mutex::new(conn),
            watcher_handle: Mutex::new(None),
            watched_path: Mutex::new(None),
        })
    }
}
