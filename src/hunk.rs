use anyhow::{Context, Result};
use sha1::{Digest, Sha1};

use crate::diff::DiffHunk;

const MAX_PREVIEW_LINES: usize = 4;

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
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
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

pub fn list_hunks(staged: bool, file: Option<&str>, commit: Option<&str>) -> Result<()> {
    let diff_output = match commit {
        Some(c) => crate::diff::run_git_diff_commit(c, file)?,
        None => crate::diff::run_git_diff(staged, file)?,
    };
    let hunks = crate::diff::parse_diff(&diff_output);
    let identified = assign_ids(&hunks);

    if identified.is_empty() {
        return Ok(());
    }

    for (id, hunk) in &identified {
        let additions = hunk.lines.iter().filter(|l| l.starts_with('+')).count();
        let deletions = hunk.lines.iter().filter(|l| l.starts_with('-')).count();

        // Extract function context from @@ header (text after the closing @@)
        let func_ctx = hunk
            .header
            .find("@@ ")
            .and_then(|start| {
                let rest = &hunk.header[start + 3..];
                rest.find("@@ ").map(|end| rest[end + 3..].trim())
            })
            .unwrap_or("");

        let func_part = if func_ctx.is_empty() {
            String::new()
        } else {
            format!(" {}", func_ctx)
        };

        println!(
            "{} {}{} (+{} -{})",
            id, hunk.file, func_part, additions, deletions
        );

        // Preview: show up to MAX_PREVIEW_LINES changed lines
        let changed: Vec<&String> = hunk
            .lines
            .iter()
            .filter(|l| l.starts_with('+') || l.starts_with('-'))
            .collect();

        let show = changed.len().min(MAX_PREVIEW_LINES);
        for line in &changed[..show] {
            println!("  {}", line);
        }
        if changed.len() > MAX_PREVIEW_LINES {
            println!("  ... (+{} more lines)", changed.len() - MAX_PREVIEW_LINES);
        }
        println!();
    }

    Ok(())
}

pub fn show_hunk(id: &str, commit: Option<&str>) -> Result<()> {
    let hunk = match commit {
        Some(c) => find_hunk_in_commit(id, c)?,
        None => find_hunk_by_id(id, false).or_else(|_| find_hunk_by_id(id, true))?,
    };

    println!("{}", hunk.header);
    for line in &hunk.lines {
        println!("{}", line);
    }
    Ok(())
}

fn find_hunk_in_commit(id: &str, commit: &str) -> Result<DiffHunk> {
    let diff_output = crate::diff::run_git_diff_commit(commit, None)?;
    let hunks = crate::diff::parse_diff(&diff_output);
    let identified = assign_ids(&hunks);
    identified
        .into_iter()
        .find(|(hunk_id, _)| hunk_id == id)
        .map(|(_, hunk)| hunk.clone())
        .ok_or_else(|| anyhow::anyhow!("hunk {} not found in commit {}", id, commit))
}

/// Find a hunk by ID in either staged or unstaged diff.
fn find_hunk_by_id(id: &str, staged: bool) -> Result<DiffHunk> {
    let diff_output = crate::diff::run_git_diff(staged, None)?;
    let hunks = crate::diff::parse_diff(&diff_output);
    let identified = assign_ids(&hunks);

    identified
        .into_iter()
        .find(|(hunk_id, _)| hunk_id == id)
        .map(|(_, hunk)| hunk.clone())
        .ok_or_else(|| anyhow::anyhow!("hunk {} not found (re-run 'hunks')", id))
}

pub enum ApplyMode {
    Stage,
    Unstage,
    Discard,
}

pub fn apply_hunks(ids: &[String], mode: ApplyMode) -> Result<()> {
    let staged = matches!(mode, ApplyMode::Unstage);
    let diff_output = crate::diff::run_git_diff(staged, None)?;
    let hunks = crate::diff::parse_diff(&diff_output);
    let identified = assign_ids(&hunks);

    let mut combined_patch = String::new();
    for id in ids {
        let (_, hunk) = identified
            .iter()
            .find(|(hunk_id, _)| hunk_id == id)
            .ok_or_else(|| anyhow::anyhow!("hunk {} not found (re-run 'hunks')", id))?;

        combined_patch.push_str(&build_patch(hunk));
        eprintln!("{}", id);
    }

    apply_patch(&combined_patch, &mode)?;
    Ok(())
}

pub fn undo_hunks(ids: &[String], commit: &str) -> Result<()> {
    let diff_output = crate::diff::run_git_diff_commit(commit, None)?;
    let hunks = crate::diff::parse_diff(&diff_output);
    let identified = assign_ids(&hunks);

    let mut combined_patch = String::new();
    for id in ids {
        let (_, hunk) = identified
            .iter()
            .find(|(hunk_id, _)| hunk_id == id)
            .ok_or_else(|| anyhow::anyhow!("hunk {} not found in commit {}", id, commit))?;

        combined_patch.push_str(&build_patch(hunk));
        eprintln!("{}", id);
    }

    apply_patch(&combined_patch, &ApplyMode::Discard)?;
    Ok(())
}

pub fn undo_files(files: &[String], commit: &str) -> Result<()> {
    let diff_output = crate::diff::run_git_diff_commit(commit, None)?;
    let hunks = crate::diff::parse_diff(&diff_output);

    let mut combined_patch = String::new();
    let mut matched_files = std::collections::HashSet::new();
    for hunk in &hunks {
        if files.iter().any(|f| f == &hunk.file) {
            combined_patch.push_str(&build_patch(hunk));
            matched_files.insert(&hunk.file);
        }
    }

    for file in files {
        if !matched_files.contains(&file) {
            anyhow::bail!("file {} not found in commit {}", file, commit);
        }
        eprintln!("{}", file);
    }

    apply_patch(&combined_patch, &ApplyMode::Discard)?;
    Ok(())
}

/// Reconstruct a minimal unified diff patch for a single hunk.
fn build_patch(hunk: &DiffHunk) -> String {
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
fn apply_patch(patch: &str, mode: &ApplyMode) -> Result<()> {
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
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(patch.as_bytes())?;
    let output = child.wait_with_output()?;

    if !output.status.success() {
        anyhow::bail!(
            "git apply failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
