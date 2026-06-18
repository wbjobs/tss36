use anyhow::{Context, Result};

pub const DEFAULT_BLOCK_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub struct FileBlock {
    pub index: usize,
    pub hash: String,
    pub data: Vec<u8>,
    pub size: usize,
}

pub fn xxh3_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let mut hash2: u64 = 0x9e3779b97f4a7c15;
    for chunk in data.chunks(8) {
        let mut val: u64 = 0;
        for (i, &b) in chunk.iter().enumerate() {
            val |= (b as u64) << (i * 8);
        }
        hash2 ^= val;
        hash2 = hash2.rotate_left(13);
        hash2 = hash2.wrapping_mul(5).wrapping_add(0xe6546b64);
    }
    format!("{:016x}{:016x}", hash, hash2)
}

pub fn split_into_blocks(data: &[u8], block_size: usize) -> Vec<FileBlock> {
    if data.is_empty() {
        return vec![FileBlock {
            index: 0,
            hash: xxh3_hash(&[]),
            data: Vec::new(),
            size: 0,
        }];
    }
    let mut blocks = Vec::new();
    let mut index = 0usize;
    let mut offset = 0usize;
    while offset < data.len() {
        let end = (offset + block_size).min(data.len());
        let chunk = &data[offset..end];
        blocks.push(FileBlock {
            index,
            hash: xxh3_hash(chunk),
            data: chunk.to_vec(),
            size: chunk.len(),
        });
        index += 1;
        offset = end;
    }
    blocks
}

#[derive(Debug, Clone)]
pub struct BlockDiffResult {
    pub added_blocks: Vec<FileBlock>,
    pub kept_block_hashes: Vec<String>,
    pub ordered_hashes: Vec<String>,
    pub total_size: usize,
    pub changed_count: usize,
    pub unchanged_count: usize,
}

pub fn compute_block_diff(
    old_blocks: &[FileBlock],
    new_blocks: &[FileBlock],
) -> BlockDiffResult {
    let old_hash_set: std::collections::HashSet<&str> = old_blocks
        .iter()
        .map(|b| b.hash.as_str())
        .collect();
    let mut added_blocks = Vec::new();
    let mut kept_block_hashes = Vec::new();
    let mut ordered_hashes = Vec::new();
    let mut changed_count = 0usize;
    let mut unchanged_count = 0usize;

    for new_block in new_blocks {
        ordered_hashes.push(new_block.hash.clone());
        if old_hash_set.contains(new_block.hash.as_str()) {
            kept_block_hashes.push(new_block.hash.clone());
            unchanged_count += 1;
        } else {
            added_blocks.push(new_block.clone());
            changed_count += 1;
        }
    }

    let total_size = new_blocks.iter().map(|b| b.size).sum();
    BlockDiffResult {
        added_blocks,
        kept_block_hashes,
        ordered_hashes,
        total_size,
        changed_count,
        unchanged_count,
    }
}

pub fn reconstruct_from_blocks(
    ordered_hashes: &[String],
    hash_to_data: &std::collections::HashMap<String, Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    for hash in ordered_hashes {
        let data = hash_to_data
            .get(hash)
            .with_context(|| format!("找不到块数据，hash={}", hash))?;
        result.extend_from_slice(data);
    }
    Ok(result)
}

pub fn hash_bytes(data: &[u8]) -> String {
    xxh3_hash(data)
}

pub fn hash_content(content: &str) -> String {
    xxh3_hash(content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_blocks_small() {
        let data = b"hello world";
        let blocks = split_into_blocks(data, DEFAULT_BLOCK_SIZE);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].size, 11);
    }

    #[test]
    fn test_split_blocks_multiple() {
        let data = vec![0u8; 200_000];
        let blocks = split_into_blocks(&data, 64 * 1024);
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].size, 65536);
        assert_eq!(blocks[1].size, 65536);
        assert_eq!(blocks[2].size, 65536);
        assert_eq!(blocks[3].size, 3392);
    }

    #[test]
    fn test_block_diff_identical() {
        let data = vec![0u8; 200_000];
        let blocks_a = split_into_blocks(&data, 64 * 1024);
        let blocks_b = split_into_blocks(&data, 64 * 1024);
        let diff = compute_block_diff(&blocks_a, &blocks_b);
        assert_eq!(diff.added_blocks.len(), 0);
        assert_eq!(diff.changed_count, 0);
        assert_eq!(diff.unchanged_count, 4);
    }

    #[test]
    fn test_block_diff_one_block_changed() {
        let mut data_a = vec![0u8; 200_000];
        let mut data_b = data_a.clone();
        for i in 0..1000 {
            data_b[65536 + i] = 0xFF;
        }
        let blocks_a = split_into_blocks(&data_a, 64 * 1024);
        let blocks_b = split_into_blocks(&data_b, 64 * 1024);
        let diff = compute_block_diff(&blocks_a, &blocks_b);
        assert_eq!(diff.added_blocks.len(), 1);
        assert_eq!(diff.changed_count, 1);
        assert_eq!(diff.unchanged_count, 3);
    }

    #[test]
    fn test_reconstruct_roundtrip() {
        let original = b"hello world this is a test file content".to_vec();
        let blocks = split_into_blocks(&original, 5);
        let mut map = std::collections::HashMap::new();
        let mut hashes = Vec::new();
        for b in &blocks {
            map.insert(b.hash.clone(), b.data.clone());
            hashes.push(b.hash.clone());
        }
        let reconstructed = reconstruct_from_blocks(&hashes, &map).unwrap();
        assert_eq!(reconstructed, original);
    }

    #[test]
    fn test_hash_deterministic() {
        let h1 = xxh3_hash(b"test data");
        let h2 = xxh3_hash(b"test data");
        assert_eq!(h1, h2);
        let h3 = xxh3_hash(b"different data");
        assert_ne!(h1, h3);
    }
}
