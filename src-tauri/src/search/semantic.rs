use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};
use crate::embedding::generate_embedding;
use crate::db::{self, SearchFilters, SemanticSearchResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    pub query: String,
    pub top_k: Option<usize>,
    pub filters: Option<SearchFilters>,
}

pub fn perform_semantic_search(
    conn: &Connection,
    request: &QueryRequest,
) -> Result<Vec<SemanticSearchResult>> {
    let query_vector = generate_embedding(&request.query);
    let top_k = request.top_k.unwrap_or(20).clamp(1, 100);
    let results = db::semantic_search(conn, &query_vector, request.filters.as_ref(), top_k)?;
    Ok(results)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub file_path: String,
    pub version_number: i64,
    pub score: f32,
    pub snippet: String,
    pub timestamp: DateTime<Local>,
    pub file_type: String,
}

impl From<SemanticSearchResult> for SearchResultItem {
    fn from(r: SemanticSearchResult) -> Self {
        Self {
            file_path: r.file_path,
            version_number: r.version_number,
            score: r.score,
            snippet: r.snippet,
            timestamp: r.timestamp,
            file_type: r.file_type,
        }
    }
}
