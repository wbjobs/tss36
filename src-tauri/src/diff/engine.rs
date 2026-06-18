use anyhow::{Context, Result};
use similar::{ChangeTag, TextDiff};
use similar::udiff::UnifiedDiff;
use crate::db::VersionRecord;

pub fn compute_diff(old: &str, new: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let udiff = UnifiedDiff::from_text_diff(&diff)
        .context_radius(3)
        .to_string();
    if udiff.is_empty() {
        String::new()
    } else {
        udiff
    }
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    let trimmed = line.strip_prefix("@@")?.strip_suffix("@@")?.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }
    let parse_part = |s: &str| -> Option<(usize, usize)> {
        let s = s.strip_prefix('-').or_else(|| s.strip_prefix('+'))?;
        if let Some((start, count)) = s.split_once(',') {
            let start: usize = start.parse().ok()?;
            let count: usize = count.parse().ok()?;
            Some((start.saturating_sub(1), count))
        } else {
            let start: usize = s.parse().ok()?;
            Some((start.saturating_sub(1), 1))
        }
    };
    let old_range = parse_part(parts[0])?;
    let new_range = parse_part(parts[1])?;
    Some((old_range.0, old_range.1, new_range.0, new_range.1))
}

pub fn apply_diff(original: &str, patch: &str) -> Result<String> {
    if patch.is_empty() {
        return Ok(original.to_string());
    }
    let mut original_lines: Vec<&str> = original.lines().collect();
    let mut result_lines: Vec<String> = Vec::new();
    let mut orig_idx = 0;
    let mut in_hunk = false;
    let patch_lines: Vec<&str> = patch.lines().collect();
    let mut i = 0;
    while i < patch_lines.len() {
        let line = patch_lines[i];
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            i += 1;
            continue;
        }
        if line.starts_with("@@") {
            if in_hunk {
                while orig_idx < original_lines.len() {
                    result_lines.push(original_lines[orig_idx].to_string());
                    orig_idx += 1;
                }
            }
            in_hunk = true;
            if let Some((old_start, _old_count, _new_start, _new_count)) = parse_hunk_header(line) {
                while orig_idx < old_start && orig_idx < original_lines.len() {
                    result_lines.push(original_lines[orig_idx].to_string());
                    orig_idx += 1;
                }
            }
            i += 1;
            continue;
        }
        if in_hunk {
            if let Some(rest) = line.strip_prefix(' ') {
                if orig_idx < original_lines.len() {
                    result_lines.push(rest.to_string());
                    orig_idx += 1;
                } else {
                    result_lines.push(rest.to_string());
                }
            } else if let Some(rest) = line.strip_prefix('-') {
                if orig_idx < original_lines.len() {
                    orig_idx += 1;
                }
                let _ = rest;
            } else if let Some(rest) = line.strip_prefix('+') {
                result_lines.push(rest.to_string());
            } else if line.starts_with('\\') {
            }
        }
        i += 1;
    }
    while orig_idx < original_lines.len() {
        result_lines.push(original_lines[orig_idx].to_string());
        orig_idx += 1;
    }
    let mut result = result_lines.join("\n");
    let orig_ends_newline = original.ends_with('\n');
    let patch_ends_newline = patch.ends_with('\n') || result_lines.is_empty();
    let _ = patch_ends_newline;
    if orig_ends_newline && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

pub fn reconstruct_content(versions_sorted: &[VersionRecord]) -> Result<String> {
    if versions_sorted.is_empty() {
        return Err(anyhow::anyhow!("版本列表为空"));
    }
    let mut content = String::new();
    for (idx, version) in versions_sorted.iter().enumerate() {
        if idx == 0 {
            if let Some(snapshot) = &version.content_snapshot {
                content = snapshot.clone();
            } else {
                content = apply_diff("", &version.diff_patch)
                    .context("应用初始版本补丁失败")?;
            }
        } else {
            content = apply_diff(&content, &version.diff_patch)
                .context(format!("应用版本 {} 补丁失败", version.version_number))?;
        }
    }
    Ok(content)
}

pub fn hash_content(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_roundtrip() {
        let original = "line1\nline2\nline3\nline4\nline5\n";
        let modified = "line1\nline2 modified\nline3\nline4 new\nline5\n";
        let diff = compute_diff(original, modified);
        let result = apply_diff(original, &diff).unwrap();
        assert_eq!(result, modified);
    }

    #[test]
    fn test_empty_diff() {
        let text = "hello\nworld\n";
        let diff = compute_diff(text, text);
        assert!(diff.is_empty());
    }
}
