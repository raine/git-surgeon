use anyhow::{Context, Result};
use std::process::Command;

/// Parse @@ header to extract line ranges.
/// Returns (old_from, old_count, new_from, new_count).
/// Count defaults to 1 if omitted (e.g., "@@ -5 +5 @@").
pub fn parse_hunk_header(header: &str) -> Option<(usize, usize, usize, usize)> {
    // Format: @@ -old_from,old_count +new_from,new_count @@ optional context
    // Count may be omitted if 1

    let header = header.strip_prefix("@@ ")?;
    let end_idx = header.find(" @@")?;
    let range_part = &header[..end_idx];

    let mut parts = range_part.split_whitespace();

    // Parse old range: -from,count or -from
    let old_part = parts.next()?.strip_prefix('-')?;
    let (old_from, old_count) = parse_range(old_part)?;

    // Parse new range: +from,count or +from
    let new_part = parts.next()?.strip_prefix('+')?;
    let (new_from, new_count) = parse_range(new_part)?;

    Some((old_from, old_count, new_from, new_count))
}

fn parse_range(s: &str) -> Option<(usize, usize)> {
    if let Some((from, count)) = s.split_once(',') {
        Some((from.parse().ok()?, count.parse().ok()?))
    } else {
        Some((s.parse().ok()?, 1))
    }
}

/// Get blame hashes for a line range in a file.
/// Returns Vec of 7-char hashes, one per line.
/// If revision is None, blames the working tree.
pub fn get_blame(
    file: &str,
    from: usize,
    count: usize,
    revision: Option<&str>,
) -> Result<Vec<String>> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let mut cmd = Command::new("git");
    cmd.args([
        "blame",
        "--line-porcelain",
        "-L",
        &format!("{},+{}", from, count),
    ]);

    if let Some(rev) = revision {
        cmd.arg(rev);
    }

    cmd.arg("--").arg(file);

    let output = cmd.output().context("failed to run git blame")?;

    if !output.status.success() {
        // Return empty vec on failure (graceful degradation)
        // Caller will use fallback "0000000" markers
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // --line-porcelain outputs one header line per blamed line
    // Header lines start with 40-char hex hash
    // Content lines start with \t, skip those to avoid false matches
    let hashes: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            // Content lines in porcelain format start with a tab
            if line.starts_with('\t') {
                return None;
            }
            let first_token = line.split_whitespace().next()?;
            // Strip leading ^ for boundary commits
            let hash = first_token.trim_start_matches('^');
            if hash.len() >= 40 && hash.chars().take(40).all(|c| c.is_ascii_hexdigit()) {
                Some(hash[..7].to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(hashes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hunk_header_full() {
        let result = parse_hunk_header("@@ -10,3 +10,4 @@ fn main");
        assert_eq!(result, Some((10, 3, 10, 4)));
    }

    #[test]
    fn test_parse_hunk_header_no_count() {
        let result = parse_hunk_header("@@ -5 +5 @@");
        assert_eq!(result, Some((5, 1, 5, 1)));
    }

    #[test]
    fn test_parse_hunk_header_mixed() {
        let result = parse_hunk_header("@@ -1,2 +1 @@");
        assert_eq!(result, Some((1, 2, 1, 1)));
    }

    #[test]
    fn test_parse_hunk_header_zero_count() {
        // New file: @@ -0,0 +1,3 @@
        let result = parse_hunk_header("@@ -0,0 +1,3 @@");
        assert_eq!(result, Some((0, 0, 1, 3)));
    }

    #[test]
    fn test_parse_hunk_header_invalid() {
        assert_eq!(parse_hunk_header("not a header"), None);
    }
}
