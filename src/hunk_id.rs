use sha1::{Digest, Sha1};
use std::collections::HashMap;

use crate::diff::DiffHunk;

/// Compute raw hash for a hunk (before collision suffix).
/// Hashes: file path + all hunk lines (context + changes).
/// Excludes @@ header line numbers so IDs survive line shifts.
fn compute_raw_id(hunk: &DiffHunk) -> String {
    let mut hasher = Sha1::new();
    hasher.update(hunk.file.as_bytes());
    for line in &hunk.lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    let result = hasher.finalize();
    hex::encode(&result[..4]) // 8 hex chars, truncate to 7 below
}

/// Assign unique IDs to hunks. Duplicates get -2, -3, etc.
pub fn assign_ids(hunks: &[DiffHunk]) -> Vec<(String, &DiffHunk)> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut result = Vec::new();

    for hunk in hunks {
        let raw = compute_raw_id(hunk);
        let id_prefix = &raw[..7];
        let count = seen.entry(id_prefix.to_string()).or_insert(0);
        *count += 1;
        let id = if *count == 1 {
            id_prefix.to_string()
        } else {
            format!("{}-{}", id_prefix, count)
        };
        result.push((id, hunk));
    }

    result
}
