use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub file_type: String,
    pub first_seen: DateTime<Local>,
    pub last_modified: DateTime<Local>,
    pub current_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRecord {
    pub id: i64,
    pub file_id: i64,
    pub version_number: i64,
    pub timestamp: DateTime<Local>,
    pub diff_patch: String,
    pub prev_content_hash: String,
    pub new_content_hash: String,
    pub content_snapshot: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRecord {
    pub id: i64,
    pub version_id: i64,
    pub vector: Vec<u8>,
    pub created_at: DateTime<Local>,
}

pub const CREATE_TABLES_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    file_type TEXT NOT NULL,
    first_seen DATETIME NOT NULL,
    last_modified DATETIME NOT NULL,
    current_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    version_number INTEGER NOT NULL,
    timestamp DATETIME NOT NULL,
    diff_patch TEXT NOT NULL,
    prev_content_hash TEXT NOT NULL,
    new_content_hash TEXT NOT NULL,
    content_snapshot TEXT,
    FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_versions_file_id ON versions(file_id);
CREATE INDEX IF NOT EXISTS idx_versions_version_number ON versions(version_number);

CREATE TABLE IF NOT EXISTS embeddings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version_id INTEGER NOT NULL,
    vector BLOB NOT NULL,
    created_at DATETIME NOT NULL,
    FOREIGN KEY (version_id) REFERENCES versions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_embeddings_version_id ON embeddings(version_id);

CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;
