pub mod schema;
pub mod operations;

pub use schema::{FileRecord, VersionRecord, EmbeddingRecord};
pub use operations::{
    init_schema, insert_file, get_file_by_path, insert_version,
    get_next_version_number, get_file_versions, get_version_content,
    insert_embedding, deserialize_vector, semantic_search,
    SearchFilters, SemanticSearchResult, upsert_metadata, get_metadata,
};
