use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::Connection;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use walkdir::WalkDir;
use path_slash::PathBufExt;

use crate::db::{self, FileRecord};
use crate::embedding::generate_embedding;

const SUPPORTED_EXTENSIONS: &[&str] = &[
    "txt", "md", "py", "js", "ts", "json", "yaml", "yml", "toml",
    "rs", "go", "java", "cpp", "c", "h", "html", "css",
];

#[derive(Debug, Clone)]
pub enum FileEventType {
    Created,
    Modified,
    Removed,
}

#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub event_type: FileEventType,
}

pub struct WatcherController {
    pub cancel_flag: Arc<AtomicBool>,
    pub join_handle: Option<tokio::task::JoinHandle<()>>,
}

impl WatcherController {
    pub fn stop(&mut self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.join_handle.take() {
            handle.abort();
        }
    }
}

fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext = ext.to_lowercase();
            SUPPORTED_EXTENSIONS.iter().any(|e| e == &ext.as_str())
        })
        .unwrap_or(false)
}

fn get_file_type(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

fn to_forward_slash_string(path: &Path) -> String {
    path.to_slash_lossy().to_string()
}

fn read_file_bytes_safe(path: &Path) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(_) => None,
    }
}

fn process_file_changed(conn: &Connection, path: &Path, event_type: FileEventType) -> Result<()> {
    let path_str = to_forward_slash_string(path);
    let file_type = get_file_type(path);

    match event_type {
        FileEventType::Removed => {
            Ok(())
        }
        FileEventType::Created | FileEventType::Modified => {
            let current_bytes = match read_file_bytes_safe(path) {
                Some(b) => b,
                None => return Ok(()),
            };

            let file_record = db::get_file_by_path(conn, &path_str)?;
            let file_id = match file_record {
                Some(ref rec) => rec.id,
                None => db::insert_file(conn, &path_str, &file_type, "", current_bytes.len() as i64)?,
            };

            let prev_version_number = match &file_record {
                Some(rec) => {
                    let versions = db::get_file_versions(conn, rec.id)?;
                    versions.last().map(|v| v.version_number)
                }
                None => None,
            };

            let (version_id, _, created) = db::create_new_version_from_content(
                conn,
                file_id,
                &path_str,
                &file_type,
                &current_bytes,
                prev_version_number,
            )?;

            if created {
                if let Ok(content_str) = String::from_utf8(current_bytes.clone()) {
                    if !content_str.trim().is_empty() {
                        let embedding = generate_embedding(&content_str);
                        if let Err(e) = db::insert_embedding(conn, version_id, &embedding) {
                            eprintln!("警告: 生成 embedding 失败: {}", e);
                        }
                    }
                }
            }

            Ok(())
        }
    }
}

pub async fn scan_initial_folder(conn: Arc<TokioMutex<Connection>>, folder_path: &Path) -> Result<()> {
    let folder_path = folder_path.to_path_buf();
    let conn_clone = Arc::clone(&conn);
    let conn_guard = conn_clone.lock().await;

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(&folder_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && is_supported_file(path) {
            files.push(path.to_path_buf());
        }
    }

    for file_path in files {
        if let Err(e) = process_file_changed(&conn_guard, &file_path, FileEventType::Created) {
            eprintln!("警告: 处理文件 {:?} 失败: {}", file_path, e);
        }
    }

    Ok(())
}

async fn debounce_events(
    mut rx: mpsc::Receiver<FileEvent>,
    conn: Arc<TokioMutex<Connection>>,
    cancel_flag: Arc<AtomicBool>,
    debounce_ms: u64,
) {
    const DEBOUNCE_INTERVAL_MS: u64 = 100;
    let mut pending: HashMap<PathBuf, FileEventType> = HashMap::new();
    let mut last_seen: HashMap<PathBuf, std::time::Instant> = HashMap::new();

    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            break;
        }
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(evt) => {
                        let path = evt.path.clone();
                        let now = std::time::Instant::now();
                        last_seen.insert(path.clone(), now);
                        let current_event = pending.entry(path.clone()).or_insert(evt.event_type.clone());
                        match (&current_event, &evt.event_type) {
                            (FileEventType::Created, FileEventType::Modified) => {}
                            (FileEventType::Created, FileEventType::Removed) => {
                                *current_event = FileEventType::Removed;
                            }
                            _ => {
                                *current_event = evt.event_type.clone();
                            }
                        }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(DEBOUNCE_INTERVAL_MS)) => {}
        }
        let now = std::time::Instant::now();
        let mut to_process: Vec<(PathBuf, FileEventType)> = Vec::new();
        pending.retain(|path, event| {
            if let Some(&last) = last_seen.get(path) {
                if now.duration_since(last) >= Duration::from_millis(debounce_ms) {
                    to_process.push((path.clone(), event.clone()));
                    return false;
                }
            }
            true
        });
        last_seen.retain(|path, _| pending.contains_key(path));

        if !to_process.is_empty() {
            let conn_guard = conn.lock().await;
            let mut dedup: HashSet<PathBuf> = HashSet::new();
            for (path, event_type) in to_process {
                if !dedup.insert(path.clone()) {
                    continue;
                }
                if !is_supported_file(&path) && !matches!(event_type, FileEventType::Removed) {
                    continue;
                }
                if let Err(e) = process_file_changed(&conn_guard, &path, event_type.clone()) {
                    eprintln!("警告: 处理文件事件失败 {:?}: {}", path, e);
                }
            }
        }
    }

    if !pending.is_empty() {
        let conn_guard = conn.lock().await;
        for (path, event_type) in pending {
            if !is_supported_file(&path) && !matches!(event_type, FileEventType::Removed) {
                continue;
            }
            if let Err(e) = process_file_changed(&conn_guard, &path, event_type) {
                eprintln!("警告: 处理剩余文件事件失败 {:?}: {}", path, e);
            }
        }
    }
}

pub fn start_watcher(
    folder_path: String,
    db_connection: Arc<TokioMutex<Connection>>,
) -> Result<WatcherController> {
    let folder = PathBuf::from(&folder_path);
    if !folder.exists() {
        return Err(anyhow::anyhow!("目录不存在: {}", folder_path));
    }
    if !folder.is_dir() {
        return Err(anyhow::anyhow!("路径不是目录: {}", folder_path));
    }

    let (tx, rx) = mpsc::channel::<FileEvent>(1000);
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel_flag);
    let cancel_clone2 = Arc::clone(&cancel_flag);
    let folder_path_clone = folder.clone();
    let conn_clone = Arc::clone(&db_connection);
    let conn_clone2 = Arc::clone(&db_connection);

    let rt = tokio::runtime::Handle::current();
    rt.spawn(async move {
        let _ = scan_initial_folder(Arc::clone(&conn_clone), &folder_path_clone).await;
    });

    rt.spawn(async move {
        debounce_events(rx, Arc::clone(&conn_clone2), cancel_clone2, 1000).await;
    });

    let watcher_folder = folder.clone();
    let cancel_flag_watcher = Arc::clone(&cancel_flag);
    let rt_for_watcher = tokio::runtime::Handle::current();
    let join_handle = std::thread::spawn(move || {
        let (watcher_tx, watcher_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
        let mut watcher: Option<RecommendedWatcher> = None;
        match notify::recommended_watcher(move |res: notify::Result<Event>| {
            let _ = watcher_tx.send(res);
        }) {
            Ok(mut w) => {
                if let Err(e) = w.watch(&watcher_folder, RecursiveMode::Recursive) {
                    eprintln!("启动 watcher 失败: {}", e);
                    return;
                }
                watcher = Some(w);
            }
            Err(e) => {
                eprintln!("创建 watcher 失败: {}", e);
                return;
            }
        }

        let local_rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("创建本地 runtime 失败: {}", e);
                return;
            }
        };

        loop {
            if cancel_flag_watcher.load(Ordering::SeqCst) {
                break;
            }
            match watcher_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Ok(event)) => {
                    let paths: Vec<PathBuf> = event.paths.clone();
                    let kind = event.kind.clone();
                    let tx_clone = tx.clone();
                    local_rt.spawn(async move {
                        for path in paths {
                            let event_type = match kind {
                                EventKind::Create(_) => FileEventType::Created,
                                EventKind::Modify(_) => FileEventType::Modified,
                                EventKind::Remove(_) => FileEventType::Removed,
                                _ => continue,
                            };
                            let _ = tx_clone.send(FileEvent {
                                path: path.clone(),
                                event_type,
                            }).await;
                        }
                    });
                }
                Ok(Err(e)) => {
                    eprintln!("watcher 错误: {}", e);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        if let Some(mut w) = watcher {
            let _ = w.unwatch(&watcher_folder);
        }
    });

    let _ = join_handle;
    let dummy_handle = tokio::spawn(async move {
        loop {
            if cancel_clone.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    Ok(WatcherController {
        cancel_flag,
        join_handle: Some(dummy_handle),
    })
}
