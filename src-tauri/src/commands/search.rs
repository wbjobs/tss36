use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;
use crate::search::semantic::{QueryRequest, perform_semantic_search};
use crate::db::SearchFilters;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFiltersDto {
    pub file_types: Option<Vec<String>>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub path_keyword: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultDto {
    pub file_path: String,
    pub version_number: i64,
    pub score: f32,
    pub snippet: String,
    pub timestamp: DateTime<Local>,
    pub file_type: String,
}

#[tauri::command]
pub fn semantic_search(
    query: String,
    filters: Option<SearchFiltersDto>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResultDto>, String> {
    let conn = state.db_connection.lock().map_err(|e| e.to_string())?;

    let search_filters = match filters {
        Some(f) => {
            let mut sf = SearchFilters {
                file_types: f.file_types,
                date_from: None,
                date_to: None,
                path_keyword: f.path_keyword,
            };
            if let Some(from) = f.date_from {
                sf.date_from = Some(
                    DateTime::parse_from_rfc3339(&from)
                        .map(|d| d.with_timezone(&Local))
                        .map_err(|e| format!("日期格式错误: {}", e))?
                );
            }
            if let Some(to) = f.date_to {
                sf.date_to = Some(
                    DateTime::parse_from_rfc3339(&to)
                        .map(|d| d.with_timezone(&Local))
                        .map_err(|e| format!("日期格式错误: {}", e))?
                );
            }
            Some(sf)
        }
        None => None,
    };

    let request = QueryRequest {
        query,
        top_k: Some(20),
        filters: search_filters,
    };

    let results = perform_semantic_search(&conn, &request)
        .map_err(|e| e.to_string())?;

    Ok(results.into_iter().map(|r| SearchResultDto {
        file_path: r.file_path,
        version_number: Some(r.version_number),
        score: r.score,
        snippet: r.snippet,
        timestamp: r.timestamp,
        file_type: r.file_type,
    }).collect())
}
