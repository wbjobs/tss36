pub mod state;
pub mod db;
pub mod diff;
pub mod watcher;
pub mod embedding;
pub mod search;
pub mod commands;

use std::path::PathBuf;

use tauri::Manager;
use anyhow::Result;

use crate::state::AppState;
use crate::db::init_schema;

fn get_app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf> {
    if let Some(dir) = app.path().app_data_dir().ok() {
        std::fs::create_dir_all(&dir)?;
        return Ok(dir);
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("docversion");
        std::fs::create_dir_all(&dir)?;
        return Ok(dir);
    }
    if let Ok(home) = std::env::var("HOME") {
        let dir = PathBuf::from(home).join(".config").join("docversion");
        std::fs::create_dir_all(&dir)?;
        return Ok(dir);
    }
    let dir = PathBuf::from(".").join("docversion_data");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = match get_app_data_dir(app.handle()) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("获取数据目录失败: {}", e);
                    PathBuf::from(".")
                }
            };
            let db_path = data_dir.join("docversion.db");
            let db_path_str = db_path.to_string_lossy().to_string();

            let state = match AppState::new(&db_path_str) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("初始化 AppState 失败: {}", e);
                    return Err(Box::new(e));
                }
            };

            {
                let conn = state.db_connection.lock().map_err(|e| {
                    let err = anyhow::anyhow!("锁定数据库失败: {}", e);
                    Box::new(err) as Box<dyn std::error::Error + Send + Sync>
                })?;
                init_schema(&conn).map_err(|e| {
                    Box::new(e) as Box<dyn std::error::Error + Send + Sync>
                })?;
            }

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_file_tree,
            commands::read_file_content,
            commands::get_file_versions,
            commands::get_file_version_content,
            commands::restore_version,
            commands::semantic_search,
            commands::watch_folder,
            commands::stop_watching,
            commands::get_watched_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
