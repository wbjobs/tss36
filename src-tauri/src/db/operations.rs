use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use rusqlite::{params, Connection};
use std::collections::HashMap;

use crate::embedding::cosine_similarity;
use super::schema::{FileRecord, VersionRecord, BlockRecord, VersionBlockRecord};
use crate::diff::{
    split_into_blocks, compute_block_diff, reconstruct_from_blocks,
    hash_content, hash_bytes, DEFAULT_BLOCK_SIZE, FileBlock,
};

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
    total_size: i64,
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
                "UPDATE files SET last_modified = ?1, current_hash = ?2, total_size = ?3 WHERE id = ?4",
                params![now, content_hash, total_size, id],
            ).context("更新文件记录失败")?;
            Ok(id)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            conn.execute(
                "INSERT INTO files (path, file_type, first_seen, last_modified, current_hash, total_size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![path, file_type, now, now, content_hash, total_size],
            ).context("插入文件记录失败")?;
            Ok(conn.last_insert_rowid())
        }
        Err(e) => Err(anyhow::anyhow!("查询文件失败: {}", e)),
    }
}

pub fn get_file_by_path(conn: &Connection, path: &str) -> Result<Option<FileRecord>> {
    let result = conn.query_row(
        "SELECT id, path, file_type, first_seen, last_modified, current_hash, total_size FROM files WHERE path = ?1",
        params![path],
        |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                file_type: row.get(2)?,
                first_seen: row.get(3)?,
                last_modified: row.get(4)?,
                current_hash: row.get(5)?,
                total_size: row.get(6)?,
            })
        },
    );
    match result {
        Ok(record) => Ok(Some(record)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("查询文件失败: {}", e)),
    }
}

pub fn insert_or_get_block(
    conn: &Connection,
    block_hash: &str,
    block_data: &[u8],
) -> Result<i64> {
    let existing = conn.query_row(
        "SELECT id FROM blocks WHERE block_hash = ?1",
        params![block_hash],
        |row| row.get::<_, i64>(0),
    );
    match existing {
        Ok(id) => {
            conn.execute(
                "UPDATE blocks SET ref_count = ref_count + 1 WHERE id = ?1",
                params![id],
            ).context("更新块引用计数失败")?;
            Ok(id)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let now: DateTime<Local> = Local::now();
            let size = block_data.len() as i64;
            conn.execute(
                "INSERT INTO blocks (block_hash, block_size, data, ref_count, created_at)
                 VALUES (?1, ?2, ?3, 1, ?4)",
                params![block_hash, size, block_data, now],
            ).context("插入块数据失败")?;
            Ok(conn.last_insert_rowid())
        }
        Err(e) => Err(anyhow::anyhow!("查询块失败: {}", e)),
    }
}

pub fn insert_version_block(
    conn: &Connection,
    version_id: i64,
    block_id: i64,
    block_index: i64,
    block_hash: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO version_blocks (version_id, block_id, block_index, block_hash)
         VALUES (?1, ?2, ?3, ?4)",
        params![version_id, block_id, block_index, block_hash],
    ).context("插入版本-块关联失败")?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Clone)]
pub struct VersionInsertResult {
    pub version_id: i64,
    pub new_blocks_stored: usize,
    pub total_blocks: usize,
}

pub fn insert_version_with_blocks(
    conn: &Connection,
    file_id: i64,
    version_number: i64,
    prev_content_hash: &str,
    new_content_hash: &str,
    content_bytes: &[u8],
    previous_blocks: &[FileBlock],
    is_full_snapshot: bool,
) -> Result<VersionInsertResult> {
    let now: DateTime<Local> = Local::now();
    let new_blocks = split_into_blocks(content_bytes, DEFAULT_BLOCK_SIZE);
    let diff = compute_block_diff(previous_blocks, &new_blocks);

    let file_size = content_bytes.len() as i64;
    let block_count = new_blocks.len() as i64;

    conn.execute(
        "INSERT INTO versions (file_id, version_number, timestamp, prev_content_hash,
                              new_content_hash, is_full_snapshot, file_size, block_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            file_id,
            version_number,
            now,
            prev_content_hash,
            new_content_hash,
            if is_full_snapshot { 1 } else { 0 },
            file_size,
            block_count,
        ],
    ).context("插入版本记录失败")?;
    let version_id = conn.last_insert_rowid();

    let mut new_blocks_stored = 0usize;
    for (idx, block) in new_blocks.iter().enumerate() {
        let block_id = insert_or_get_block(conn, &block.hash, &block.data)?;
        if !diff.kept_block_hashes.contains(&block.hash) {
            new_blocks_stored += 1;
        }
        insert_version_block(conn, version_id, block_id, idx as i64, &block.hash)?;
    }

    Ok(VersionInsertResult {
        version_id,
        new_blocks_stored,
        total_blocks: new_blocks.len(),
    })
}

pub fn insert_version(
    conn: &Connection,
    file_id: i64,
    version_number: i64,
    _diff_patch: &str,
    prev_content_hash: &str,
    new_content_hash: &str,
    _content_snapshot: Option<&str>,
) -> Result<i64> {
    let now: DateTime<Local> = Local::now();
    conn.execute(
        "INSERT INTO versions (file_id, version_number, timestamp, prev_content_hash,
                              new_content_hash, is_full_snapshot, file_size, block_count)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, 0)",
        params![
            file_id,
            version_number,
            now,
            prev_content_hash,
            new_content_hash,
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
        "SELECT id, file_id, version_number, timestamp, prev_content_hash,
                new_content_hash, is_full_snapshot, file_size, block_count
         FROM versions WHERE file_id = ?1 ORDER BY version_number ASC"
    )?;
    let rows = stmt.query_map(params![file_id], |row| {
        Ok(VersionRecord {
            id: row.get(0)?,
            file_id: row.get(1)?,
            version_number: row.get(2)?,
            timestamp: row.get(3)?,
            prev_content_hash: row.get(4)?,
            new_content_hash: row.get(5)?,
            is_full_snapshot: row.get::<_, i64>(6)? != 0,
            file_size: row.get(7)?,
            block_count: row.get(8)?,
        })
    })?;
    let mut versions = Vec::new();
    for row in rows {
        versions.push(row?);
    }
    Ok(versions)
}

pub fn get_version_block_hashes(
    conn: &Connection,
    version_id: i64,
) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT block_index, block_hash FROM version_blocks
         WHERE version_id = ?1 ORDER BY block_index ASC"
    )?;
    let rows = stmt.query_map(params![version_id], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn get_blocks_by_hashes(
    conn: &Connection,
    hashes: &[String],
) -> Result<HashMap<String, Vec<u8>>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders: Vec<String> = (1..=hashes.len()).map(|i| format!("?{}", i)).collect();
    let sql = format!(
        "SELECT block_hash, data FROM blocks WHERE block_hash IN ({})",
        placeholders.join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = hashes
        .iter()
        .map(|h| h as &dyn rusqlite::ToSql)
        .collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(params_refs), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (h, d) = row?;
        map.insert(h, d);
    }
    Ok(map)
}

pub fn reconstruct_version_content(
    conn: &Connection,
    version_id: i64,
) -> Result<Vec<u8>> {
    let block_entries = get_version_block_hashes(conn, version_id)?;
    if block_entries.is_empty() {
        return Ok(Vec::new());
    }
    let hashes: Vec<String> = block_entries.iter().map(|(_, h)| h.clone()).collect();
    let block_data = get_blocks_by_hashes(conn, &hashes)?;
    let ordered_hashes: Vec<String> = block_entries
        .into_iter()
        .map(|(_, h)| h)
        .collect();
    reconstruct_from_blocks(&ordered_hashes, &block_data)
        .context("从块重组文件内容失败")
}

pub fn get_version_content(
    conn: &Connection,
    file_id: i64,
    version_number: i64,
) -> Result<String> {
    let version_id = conn.query_row(
        "SELECT id FROM versions WHERE file_id = ?1 AND version_number = ?2",
        params![file_id, version_number],
        |row| row.get::<_, i64>(0),
    ).context("未找到指定版本")?;

    let bytes = reconstruct_version_content(conn, version_id)?;
    String::from_utf8(bytes).context("文件内容不是有效的 UTF-8")
}

pub fn get_version_blocks_for_diff(
    conn: &Connection,
    file_id: i64,
    version_number: i64,
) -> Result<Vec<FileBlock>> {
    let version_id_opt = conn.query_row(
        "SELECT id FROM versions WHERE file_id = ?1 AND version_number = ?2",
        params![file_id, version_number],
        |row| row.get::<_, i64>(0),
    );
    let version_id = match version_id_opt {
        Ok(id) => id,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(Vec::new()),
        Err(e) => return Err(anyhow::anyhow!("查询版本失败: {}", e)),
    };

    let block_entries = get_version_block_hashes(conn, version_id)?;
    if block_entries.is_empty() {
        return Ok(Vec::new());
    }
    let hashes: Vec<String> = block_entries.iter().map(|(_, h)| h.clone()).collect();
    let block_data = get_blocks_by_hashes(conn, &hashes)?;

    let mut result = Vec::new();
    for (idx, hash) in block_entries.iter().enumerate() {
        if let Some(data) = block_data.get(&hash.1) {
            result.push(FileBlock {
                index: idx,
                hash: hash.1.clone(),
                data: data.clone(),
                size: data.len(),
            });
        }
    }
    Ok(result)
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
        "SELECT f.path, f.file_type, v.version_number, v.timestamp, v.id, e.vector
         FROM embeddings e
         JOIN versions v ON e.version_id = v.id
         JOIN files f ON v.file_id = f.id
         WHERE 1=1"
    );
    let mut conditions: Vec<String> = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut param_idx = 1usize;

    if let Some(f) = filters {
        if let Some(types) = &f.file_types {
            if !types.is_empty() {
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
            row.get::<_, i64>(4)?,
            vector_bytes,
        ))
    })?;

    let mut results: Vec<(SemanticSearchResult, f32)> = Vec::new();
    for row_result in rows {
        let (path, file_type, ver_num, ts, version_id, vector_bytes) = row_result?;
        let vector = match deserialize_vector(&vector_bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let score = cosine_similarity(query_vector, &vector);
        if !score.is_finite() {
            continue;
        }
        let snippet = match reconstruct_version_content(conn, version_id) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                let text = text.trim().to_string();
                if text.len() > 200 {
                    let end = text.char_indices().nth(200).map(|(i, _)| i).unwrap_or(200);
                    format!("{}...", &text[..end])
                } else {
                    text
                }
            }
            Err(_) => String::new(),
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

pub fn create_new_version_from_content(
    conn: &Connection,
    file_id: i64,
    path: &str,
    file_type: &str,
    content_bytes: &[u8],
    prev_version_number: Option<i64>,
) -> Result<(i64, String, bool)> {
    let new_hash = hash_bytes(content_bytes);
    let file_record_opt = get_file_by_path(conn, path)?;
    let (prev_hash, prev_blocks) = match file_record_opt {
        Some(rec) => {
            let blocks = if let Some(prev_ver) = prev_version_number {
                get_version_blocks_for_diff(conn, file_id, prev_ver)?
            } else {
                Vec::new()
            };
            (rec.current_hash.clone(), blocks)
        }
        None => (String::new(), Vec::new()),
    };

    if !prev_hash.is_empty() && prev_hash == new_hash {
        return Ok((-1, new_hash, false));
    }

    let version_number = get_next_version_number(conn, file_id)?;
    let is_full_snapshot = version_number == 1 || version_number % 10 == 0;

    let result = insert_version_with_blocks(
        conn,
        file_id,
        version_number,
        &prev_hash,
        &new_hash,
        content_bytes,
        &prev_blocks,
        is_full_snapshot,
    )?;

    insert_file(conn, path, file_type, &new_hash, content_bytes.len() as i64)?;

    Ok((result.version_id, new_hash, true))
}

pub use hash_content as _hash_content_compat;
