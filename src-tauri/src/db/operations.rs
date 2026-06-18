use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use rusqlite::{params, Connection};
use crate::embedding::cosine_similarity;
use super::schema::{FileRecord, VersionRecord, EmbeddingRecord};
use crate::diff::engine::reconstruct_content;

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(super::schema::CREATE_TABLES_SQL)
        .context("初始化数据库表失败")?;
    Ok(())
}

pub fn insert_file(
    conn: &Connection,
    path: &str,
    file_type: &str,
    content_hash: &str,
) -> Result<i64> {
    let now: DateTime<Local> = Local::now();
    let existing = conn.query_row(
        "SELECT id FROM files WHERE path = ?1",
        params![path],
        |row| row.get::<_, i64>(0),
    );
    match existing {
        Ok(id) => {
            conn.execute(
                "UPDATE files SET last_modified = ?1, current_hash = ?2 WHERE id = ?3",
                params![now, content_hash, id],
            ).context("更新文件记录失败")?;
            Ok(id)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            conn.execute(
                "INSERT INTO files (path, file_type, first_seen, last_modified, current_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![path, file_type, now, now, content_hash],
            ).context("插入文件记录失败")?;
            Ok(conn.last_insert_rowid())
        }
        Err(e) => Err(anyhow::anyhow!("查询文件失败: {}", e)),
    }
}

pub fn get_file_by_path(conn: &Connection, path: &str) -> Result<Option<FileRecord>> {
    let result = conn.query_row(
        "SELECT id, path, file_type, first_seen, last_modified, current_hash FROM files WHERE path = ?1",
        params![path],
        |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                file_type: row.get(2)?,
                first_seen: row.get(3)?,
                last_modified: row.get(4)?,
                current_hash: row.get(5)?,
            })
        },
    );
    match result {
        Ok(record) => Ok(Some(record)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("查询文件失败: {}", e)),
    }
}

pub fn insert_version(
    conn: &Connection,
    file_id: i64,
    version_number: i64,
    diff_patch: &str,
    prev_content_hash: &str,
    new_content_hash: &str,
    content_snapshot: Option<&str>,
) -> Result<i64> {
    let now: DateTime<Local> = Local::now();
    conn.execute(
        "INSERT INTO versions (file_id, version_number, timestamp, diff_patch,
                              prev_content_hash, new_content_hash, content_snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            file_id,
            version_number,
            now,
            diff_patch,
            prev_content_hash,
            new_content_hash,
            content_snapshot
        ],
    ).context("插入版本记录失败")?;
    Ok(conn.last_insert_rowid())
}

pub fn get_next_version_number(conn: &Connection, file_id: i64) -> Result<i64> {
    let result: Option<i64> = conn.query_row(
        "SELECT MAX(version_number) FROM versions WHERE file_id = ?1",
        params![file_id],
        |row| row.get(0),
    ).unwrap_or(None);
    Ok(result.unwrap_or(0) + 1)
}

pub fn get_file_versions(conn: &Connection, file_id: i64) -> Result<Vec<VersionRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, file_id, version_number, timestamp, diff_patch,
                prev_content_hash, new_content_hash, content_snapshot
         FROM versions WHERE file_id = ?1 ORDER BY version_number ASC"
    )?;
    let rows = stmt.query_map(params![file_id], |row| {
        Ok(VersionRecord {
            id: row.get(0)?,
            file_id: row.get(1)?,
            version_number: row.get(2)?,
            timestamp: row.get(3)?,
            diff_patch: row.get(4)?,
            prev_content_hash: row.get(5)?,
            new_content_hash: row.get(6)?,
            content_snapshot: row.get(7)?,
        })
    })?;
    let mut versions = Vec::new();
    for row in rows {
        versions.push(row?);
    }
    Ok(versions)
}

pub fn get_version_content(conn: &Connection, file_id: i64, version_number: i64) -> Result<String> {
    let versions = get_file_versions(conn, file_id)?;
    let relevant_versions: Vec<VersionRecord> = versions
        .into_iter()
        .take_while(|v| v.version_number <= version_number)
        .collect();
    if relevant_versions.is_empty() {
        return Err(anyhow::anyhow!("未找到版本记录"));
    }
    reconstruct_content(&relevant_versions)
}

pub fn insert_embedding(
    conn: &Connection,
    version_id: i64,
    vector: &[f32],
) -> Result<i64> {
    let bytes: Vec<u8> = vector.iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let now: DateTime<Local> = Local::now();
    conn.execute(
        "INSERT INTO embeddings (version_id, vector, created_at) VALUES (?1, ?2, ?3)",
        params![version_id, bytes, now],
    ).context("插入向量记录失败")?;
    Ok(conn.last_insert_rowid())
}

pub fn deserialize_vector(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() % 4 != 0 {
        return Err(anyhow::anyhow!("向量字节长度不正确"));
    }
    let mut vector = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks(4) {
        let arr: [u8; 4] = chunk.try_into().map_err(|_| anyhow::anyhow!("转换失败"))?;
        vector.push(f32::from_le_bytes(arr));
    }
    Ok(vector)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchFilters {
    pub file_types: Option<Vec<String>>,
    pub date_from: Option<DateTime<Local>>,
    pub date_to: Option<DateTime<Local>>,
    pub path_keyword: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SemanticSearchResult {
    pub file_path: String,
    pub version_number: i64,
    pub score: f32,
    pub snippet: String,
    pub timestamp: DateTime<Local>,
    pub file_type: String,
}

pub fn semantic_search(
    conn: &Connection,
    query_vector: &[f32],
    filters: Option<&SearchFilters>,
    top_k: usize,
) -> Result<Vec<SemanticSearchResult>> {
    let mut sql = String::from(
        "SELECT f.path, f.file_type, v.version_number, v.timestamp,
                v.content_snapshot, e.vector
         FROM embeddings e
         JOIN versions v ON e.version_id = v.id
         JOIN files f ON v.file_id = f.id
         WHERE 1=1"
    );
    let mut conditions: Vec<String> = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut param_idx = 1;

    if let Some(f) = filters {
        if let Some(types) = &f.file_types {
            if !types.is_empty() {
                let placeholders: Vec<String> = (0..types.len())
                    .map(|_| format!("?{}", param_idx + { let r = param_idx; param_idx += 1; r }))
                    .collect();
                let _ = placeholders;
                let mut ph = Vec::new();
                for t in types {
                    ph.push(format!("?{}", param_idx));
                    params_vec.push(Box::new(t.clone()));
                    param_idx += 1;
                }
                conditions.push(format!("f.file_type IN ({})", ph.join(", ")));
            }
        }
        if let Some(from) = &f.date_from {
            conditions.push(format!("v.timestamp >= ?{}", param_idx));
            params_vec.push(Box::new(*from));
            param_idx += 1;
        }
        if let Some(to) = &f.date_to {
            conditions.push(format!("v.timestamp <= ?{}", param_idx));
            params_vec.push(Box::new(*to));
            param_idx += 1;
        }
        if let Some(kw) = &f.path_keyword {
            conditions.push(format!("f.path LIKE ?{}", param_idx));
            params_vec.push(Box::new(format!("%{}%", kw)));
            param_idx += 1;
        }
    }

    for c in &conditions {
        sql.push_str(" AND ");
        sql.push_str(c);
    }

    let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
        .iter()
        .map(|b| b.as_ref())
        .collect();

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_refs), |row| {
        let vector_bytes: Vec<u8> = row.get(5)?;
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, DateTime<Local>>(3)?,
            row.get::<_, Option<String>>(4)?,
            vector_bytes,
        ))
    })?;

    let mut results: Vec<(SemanticSearchResult, f32)> = Vec::new();
    for row_result in rows {
        let (path, file_type, ver_num, ts, snapshot, vector_bytes) = row_result?;
        let vector = match deserialize_vector(&vector_bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let score = cosine_similarity(query_vector, &vector);
        if !score.is_finite() {
            continue;
        }
        let snippet = match &snapshot {
            Some(s) => {
                let s = s.trim();
                if s.len() > 200 {
                    let end = s.char_indices().nth(200).map(|(i, _)| i).unwrap_or(200);
                    format!("{}...", &s[..end])
                } else {
                    s.to_string()
                }
            }
            None => String::new(),
        };
        results.push((
            SemanticSearchResult {
                file_path: path,
                version_number: ver_num,
                score,
                snippet,
                timestamp: ts,
                file_type,
            },
            score,
        ));
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    Ok(results.into_iter().map(|(r, _)| r).collect())
}

pub fn upsert_metadata(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    ).context("写入元数据失败")?;
    Ok(())
}

pub fn get_metadata(conn: &Connection, key: &str) -> Result<Option<String>> {
    let result = conn.query_row(
        "SELECT value FROM metadata WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("查询元数据失败: {}", e)),
    }
}
