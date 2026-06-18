use std::path::PathBuf;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tauri::State;
use path_slash::PathBufExt;

use crate::state::AppState;
use crate::db::{self, VersionRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub file_path: String,
    pub version_number: i64,
    pub timestamp: DateTime<Local>,
    pub diff_patch: String,
    pub size: i64,
    pub message: Option<String>,
}

fn build_version_info(v: VersionRecord, file_path: &str) -> VersionInfo {
    let size_estimate = v.content_snapshot.as_ref()
        .map(|s| s.len() as i64)
        .unwrap_or_else(|| v.diff_patch.len() as i64);
    VersionInfo {
        id: v.id.to_string(),
        file_path: file_path.to_string(),
        version_number: v.version_number,
        timestamp: v.timestamp,
        diff_patch: v.diff_patch,
        size: size_estimate,
        message: if v.version_number == 1 {
            Some("初始版本".to_string())
        } else if v.content_snapshot.is_some() {
            Some(format!("快照版本 #{}", v.version_number))
        } else {
            None
        },
    }
}

#[tauri::command]
pub fn get_file_versions(
    path: String,
    state: State<'_, AppState>,
) -> Result<Vec<VersionInfo>, String> {
    let conn = state.db_connection.lock().map_err(|e| e.to_string())?;
    let file_path = PathBuf::from(&path);
    let path_str = file_path.to_slash_lossy().to_string();

    let file = db::get_file_by_path(&conn, &path_str)
        .map_err(|e| e.to_string())?;
    let file = match file {
        Some(f) => f,
        None => return Ok(Vec::new()),
    };

    let versions = db::get_file_versions(&conn, file.id)
        .map_err(|e| e.to_string())?;
    Ok(versions.into_iter()
        .rev()
        .map(|v| build_version_info(v, &path))
        .collect())
}

#[tauri::command]
pub fn get_file_version_content(
    path: String,
    version: i64,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let conn = state.db_connection.lock().map_err(|e| e.to_string())?;
    let file_path = PathBuf::from(&path);
    let path_str = file_path.to_slash_lossy().to_string();

    let file = db::get_file_by_path(&conn, &path_str)
        .map_err(|e| e.to_string())?;
    let file = match file {
        Some(f) => f,
        None => return Err(format!("文件未在数据库中: {}", path)),
    };

    db::get_version_content(&conn, file.id, version)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restore_version(
    path: String,
    version: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db_connection.lock().map_err(|e| e.to_string())?;
    let file_path = PathBuf::from(&path);
    let path_str = file_path.to_slash_lossy().to_string();

    let file = db::get_file_by_path(&conn, &path_str)
        .map_err(|e| e.to_string())?;
    let file = match file {
        Some(f) => f,
        None => return Err(format!("文件未在数据库中: {}", path)),
    };

    let content = db::get_version_content(&conn, file.id, version)
        .map_err(|e| e.to_string())?;

    drop(conn);

    std::fs::write(&file_path, content)
        .map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(())
}
