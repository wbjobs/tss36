pub mod engine;

pub use engine::{
    FileBlock, BlockDiffResult, DEFAULT_BLOCK_SIZE,
    split_into_blocks, compute_block_diff, reconstruct_from_blocks,
    xxh3_hash, hash_content, hash_bytes,
};
