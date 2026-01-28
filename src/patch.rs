use anyhow::{Context, Result};

use crate::diff::DiffHunk;

pub enum ApplyMode {
    Stage,
    Unstage,
    Discard,
}

/// Slice a hunk to only include changes within the given 1-based line range.
/// Lines outside the range have their changes neutralized:
/// - excluded '+' lines are dropped
/// - excluded '-' lines become context (the deletion is kept)
///
/// Context lines are always preserved for patch validity.
pub fn slice_hunk(hunk: &DiffHunk, start: usize, end: usize, reverse: bool) -> Result<DiffHunk> {
    slice_hunk_multi(hunk, &[(start, end)], reverse)
}

/// Slice a hunk keeping changes from any of the given 1-based line ranges.
pub fn slice_hunk_multi(
    hunk: &DiffHunk,
    ranges: &[(usize, usize)],
    reverse: bool,
) -> Result<DiffHunk> {
    let in_any_range = |idx: usize| ranges.iter().any(|(s, e)| idx >= *s && idx <= *e);

    let mut new_lines = Vec::new();
    for (i, line) in hunk.lines.iter().enumerate() {
        let idx = i + 1;
        let in_range = in_any_range(idx);

        if let Some(rest) = line.strip_prefix('+') {
            if in_range {
                new_lines.push(line.clone());
            } else if reverse {
                new_lines.push(format!(" {}", rest));
            }
        } else if let Some(rest) = line.strip_prefix('-') {
            if in_range {
                new_lines.push(line.clone());
            } else if !reverse {
                new_lines.push(format!(" {}", rest));
            }
        } else {
            new_lines.push(line.clone());
        }
    }

    let old_count = new_lines
        .iter()
        .filter(|l| l.starts_with('-') || l.starts_with(' '))
        .count();
    let new_count = new_lines
        .iter()
        .filter(|l| l.starts_with('+') || l.starts_with(' '))
        .count();

    let (old_start, new_start) = parse_hunk_starts(&hunk.header)?;

    let func_ctx = hunk
        .header
        .find("@@ ")
        .and_then(|s| {
            let rest = &hunk.header[s + 3..];
            rest.find("@@").map(|e| &rest[e + 2..])
        })
        .unwrap_or("");

    let new_header = format!(
        "@@ -{},{} +{},{} @@{}",
        old_start, old_count, new_start, new_count, func_ctx
    );

    Ok(DiffHunk {
        file: hunk.file.clone(),
        old_file: hunk.old_file.clone(),
        new_file: hunk.new_file.clone(),
        file_header: hunk.file_header.clone(),
        header: new_header,
        lines: new_lines,
        unsupported_metadata: hunk.unsupported_metadata.clone(),
    })
}

fn parse_hunk_starts(header: &str) -> Result<(usize, usize)> {
    let content = header
        .trim_start_matches("@@ ")
        .split(" @@")
        .next()
        .ok_or_else(|| anyhow::anyhow!("invalid hunk header"))?;
    let mut parts = content.split_whitespace();
    let old_start: usize = parts
        .next()
        .and_then(|s| s.strip_prefix('-'))
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("cannot parse old start from header"))?;
    let new_start: usize = parts
        .next()
        .and_then(|s| s.strip_prefix('+'))
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| anyhow::anyhow!("cannot parse new start from header"))?;
    Ok((old_start, new_start))
}

/// Reconstruct a minimal unified diff patch for a single hunk.
pub fn build_patch(hunk: &DiffHunk) -> String {
    let mut patch = String::new();
    patch.push_str(&hunk.file_header);
    patch.push('\n');
    patch.push_str(&hunk.header);
    patch.push('\n');
    for line in &hunk.lines {
        patch.push_str(line);
        patch.push('\n');
    }
    patch
}

/// Apply a patch using git apply.
pub fn apply_patch(patch: &str, mode: &ApplyMode) -> Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("git");
    cmd.arg("apply");

    match mode {
        ApplyMode::Stage => {
            cmd.arg("--cached");
        }
        ApplyMode::Unstage => {
            cmd.arg("--cached").arg("--reverse");
        }
        ApplyMode::Discard => {
            cmd.arg("--reverse");
        }
    }

    cmd.stdin(Stdio::piped());
    let mut child = cmd.spawn().context("failed to run git apply")?;
    child.stdin.as_mut().unwrap().write_all(patch.as_bytes())?;
    let output = child.wait_with_output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git apply failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
