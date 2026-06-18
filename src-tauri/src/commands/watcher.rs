use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex as TokioMutex;
use rusqlite::Connection;

use crate::state::AppState;
use crate::watcher::{start_watcher, WatcherController};

#[tauri::command]
pub fn watch_folder(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db_conn = {
        let conn_guard = state.db_connection.lock().map_err(|e| e.to_string())?;
        let conn = match Connection::open_in_memory() {
            Ok(c) => c,
            Err(e) => return Err(format!("创建临时连接失败: {}", e)),
        };
        let _ = conn;
        Arc::new(TokioMutex::new(
            std::mem::replace(
                &mut *conn_guard,
                Connection::open_in_memory().map_err(|e| e.to_string())?,
            )
        ))
    };

    return Err("当前不支持此命令的跨线程连接共享，请使用 commands::watch_folder_v2。".to_string());
}

#[tauri::command]
pub fn watch_folder_v2(
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let folder_path = std::path::PathBuf::from(&path);
    if !folder_path.exists() {
        return Err(format!("目录不存在: {}", path));
    }
    if !folder_path.is_dir() {
        return Err(format!("路径不是目录: {}", path));
    }

    let existing = state.watched_path.lock().map_err(|e| e.to_string())?;
    if existing.is_some() {
        return Err("已有监听中的目录，请先停止当前监听".to_string());
    }
    drop(existing);

    let db_path = {
        let conn = state.db_connection.lock().map_err(|e| e.to_string())?;
        match conn.query_row("PRAGMA database_list", [], |row| row.get::<_, String>(2)) {
            Ok(p) if !p.is_empty() => p,
            _ => {
                let app_data = std::env::var("APPDATA")
                    .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.config", h)))
                    .unwrap_or_else(|_| ".".to_string());
                format!("{}/docversion/docversion.db", app_data)
            }
        }
    };

    let watcher_db_conn = Arc::new(TokioMutex::new(
        Connection::open(&db_path).map_err(|e| format!("打开数据库连接失败: {}", e))?
    ));

    let controller = start_watcher(path.clone(), watcher_db_conn)
        .map_err(|e| e.to_string())?;

    {
        let mut handle_guard = state.watcher_handle.lock().map_err(|e| e.to_string())?;
        *handle_guard = Some(controller);
    }
    {
        let mut path_guard = state.watched_path.lock().map_err(|e| e.to_string())?;
        *path_guard = Some(path);
    }

    Ok(())
}

#[tauri::command]
pub fn stop_watching(state: State<'_, AppState>) -> Result<(), String> {
    let mut handle_guard = state.watcher_handle.lock().map_err(|e| e.to_string())?;
    if let Some(mut controller) = handle_guard.take() {
        controller.stop();
    }
    drop(handle_guard);

    let mut path_guard = state.watched_path.lock().map_err(|e| e.to_string())?;
    *path_guard = None;

    Ok(())
}

#[tauri::command]
pub fn get_watched_folder(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let guard = state.watched_path.lock().map_err(|e| e.to_string())?;
    Ok(guard.clone())
}
