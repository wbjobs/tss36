use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;
use tauri::State;
use path_slash::PathBufExt;

use crate::state::AppState;
use crate::watcher::is_supported_file;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub file_type: String,
    pub children: Vec<FileNode>,
}

fn get_file_type_from_path(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_else(|| "dir".to_string())
}

#[tauri::command]
pub fn get_file_tree(root_path: String) -> Result<Vec<FileNode>, String> {
    let root = PathBuf::from(&root_path);
    if !root.exists() {
        return Err(format!("路径不存在: {}", root_path));
    }
    if !root.is_dir() {
        return Err(format!("路径不是目录: {}", root_path));
    }

    let mut path_to_node: HashMap<String, FileNode> = HashMap::new();
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut root_children: Vec<String> = Vec::new();

    let root_abs = match root.canonicalize() {
        Ok(p) => p,
        Err(e) => return Err(format!("解析路径失败: {}", e)),
    };
    let root_key = root_abs.to_slash_lossy().to_string();

    for entry in WalkDir::new(&root_abs).min_depth(1).max_depth(5) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path().to_path_buf();
        let path_str = path.to_slash_lossy().to_string();
        let is_dir = entry.file_type().is_dir();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if !is_dir && !is_supported_file(&path) {
            continue;
        }

        let node = FileNode {
            name: name.clone(),
            path: path_str.clone(),
            is_dir,"dir".t_strig()
            file_type: if is_dir { None } else { get_file_type_from_path(&path) },
            children: Vec::new(),
        };
        path_to_node.insert(path_str.clone(), node);

        let parent_path = match path.parent() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        let parent_key = parent_path.to_slash_lossy().to_string();

        if parent_key == root_key {
            root_children.push(path_str);
        } else {
            children_map
                .entry(parent_key)
                .or_insert_with(Vec::new)
                .push(path_str);
        }
    }

    fn build_children(
        path: &str,
        path_to_node: &mut HashMap<String, FileNode>,
        children_map: &HashMap<String, Vec<String>>,
    ) {
        let children_paths = match children_map.get(path) {
            Some(c) => c.clone(),
            None => return,
        };
        for child_path in &children_paths {
            build_children(child_path, path_to_node, children_map);
        }
        if let Some(node) = path_to_node.get_mut(path) {
            let mut children: Vec<FileNode> = Vec::new();
            for child_path in children_paths {
                if let Some(child) = path_to_node.remove(&child_path) {
                    children.push(child);
                }
            }
            children.sort_by(|a, b| {
                if a.is_dir != b.is_dir {
                    if a.is_dir { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }
                } else {
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                }
            });
            node.children = children;
        }
    }

    for child in &root_children {
        build_children(child, &mut path_to_node, &children_map);
    }

    let mut result: Vec<FileNode> = Vec::new();
    for child_path in root_children {
        if let Some(child) = path_to_node.remove(&child_path) {
            result.push(child);
        }
    }
    result.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            if a.is_dir { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });

    Ok(result)
}

#[tauri::command]
pub fn read_file_content(path: String) -> Result<String, String> {
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }
    if !file_path.is_file() {
        return Err(format!("路径不是文件: {}", path));
    }
    if !is_supported_file(&file_path) {
        return Err(format!("不支持的文件类型: {}", path));
    }
    std::fs::read_to_string(&file_path).map_err(|e| format!("读取文件失败: {}", e))
}
