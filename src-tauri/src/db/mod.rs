pub mod schema;
pub mod operations;

pub use schema::{
    FileRecord, VersionRecord, BlockRecord, VersionBlockRecord, EmbeddingRecord,
};
pub use operations::{
    init_schema,
    insert_file, get_file_by_path,
    insert_version, insert_version_with_blocks, get_next_version_number,
    get_file_versions, get_version_content,
    insert_or_get_block, insert_version_block, get_version_block_hashes, get_blocks_by_hashes,
    reconstruct_version_content, create_new_version_from_content,
    get_version_blocks_for_diff,
    insert_embedding, deserialize_vector, semantic_search,
    SearchFilters, SemanticSearchResult, VersionInsertResult,
    upsert_metadata, get_metadata,
};
