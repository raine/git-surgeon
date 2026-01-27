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
    let width = hunk.lines.len().to_string().len();
    for (i, line) in hunk.lines.iter().enumerate() {
        println!("{:>w$}:{}", i + 1, line, w = width);
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

pub fn apply_hunks(ids: &[String], mode: ApplyMode, lines: Option<(usize, usize)>) -> Result<()> {
    if lines.is_some() && ids.len() != 1 {
        anyhow::bail!("--lines requires exactly one hunk ID");
    }

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

        let reverse = matches!(mode, ApplyMode::Unstage | ApplyMode::Discard);
        let patched_hunk = if let Some((start, end)) = lines {
            slice_hunk(hunk, start, end, reverse)?
        } else {
            (*hunk).clone()
        };
        combined_patch.push_str(&build_patch(&patched_hunk));
        eprintln!("{}", id);
    }

    apply_patch(&combined_patch, &mode)?;
    Ok(())
}

pub fn undo_hunks(ids: &[String], commit: &str, lines: Option<(usize, usize)>) -> Result<()> {
    if lines.is_some() && ids.len() != 1 {
        anyhow::bail!("--lines requires exactly one hunk ID");
    }

    let diff_output = crate::diff::run_git_diff_commit(commit, None)?;
    let hunks = crate::diff::parse_diff(&diff_output);
    let identified = assign_ids(&hunks);

    let mut combined_patch = String::new();
    for id in ids {
        let (_, hunk) = identified
            .iter()
            .find(|(hunk_id, _)| hunk_id == id)
            .ok_or_else(|| anyhow::anyhow!("hunk {} not found in commit {}", id, commit))?;

        let patched_hunk = if let Some((start, end)) = lines {
            slice_hunk(hunk, start, end, true)?
        } else {
            (*hunk).clone()
        };
        combined_patch.push_str(&build_patch(&patched_hunk));
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

/// Slice a hunk to only include changes within the given 1-based line range.
/// Lines outside the range have their changes neutralized:
/// - excluded '+' lines are dropped
/// - excluded '-' lines become context (the deletion is kept)
/// Context lines are always preserved for patch validity.
fn slice_hunk(hunk: &DiffHunk, start: usize, end: usize, reverse: bool) -> Result<DiffHunk> {
    let mut new_lines = Vec::new();
    for (i, line) in hunk.lines.iter().enumerate() {
        let idx = i + 1; // 1-based
        let in_range = idx >= start && idx <= end;

        if line.starts_with('+') {
            if in_range {
                new_lines.push(line.clone());
            } else if reverse {
                // For reverse apply: excluded '+' lines exist in the file,
                // so they must become context for the patch to match.
                new_lines.push(format!(" {}", &line[1..]));
            }
            // For forward apply: excluded '+' lines are simply dropped
        } else if line.starts_with('-') {
            if in_range {
                new_lines.push(line.clone());
            } else if !reverse {
                // For forward apply: excluded '-' lines become context
                new_lines.push(format!(" {}", &line[1..]));
            }
            // For reverse apply: excluded '-' lines don't exist in the file, drop them
        } else {
            // context lines: always keep
            new_lines.push(line.clone());
        }
    }

    // Recompute @@ header
    let old_count = new_lines
        .iter()
        .filter(|l| l.starts_with('-') || l.starts_with(' '))
        .count();
    let new_count = new_lines
        .iter()
        .filter(|l| l.starts_with('+') || l.starts_with(' '))
        .count();

    // Parse original start lines from header
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
        file_header: hunk.file_header.clone(),
        header: new_header,
        lines: new_lines,
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

/// Fold currently staged changes into an earlier commit via autosquash rebase.
/// If the target is HEAD, uses simple --amend instead.
pub fn fixup(commit: &str) -> Result<()> {
    use std::process::Command;

    // Verify there are staged changes
    let status = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .status()
        .context("failed to run git diff")?;
    if status.success() {
        anyhow::bail!("no staged changes to fixup");
    }

    // Check no rebase/cherry-pick in progress
    for dir_name in ["rebase-merge", "rebase-apply"] {
        let check = Command::new("git")
            .args(["rev-parse", "--git-path", dir_name])
            .output()
            .context("failed to check rebase state")?;
        let dir = String::from_utf8_lossy(&check.stdout).trim().to_string();
        if std::path::Path::new(&dir).exists() {
            anyhow::bail!("rebase already in progress");
        }
    }

    // Resolve the target commit SHA
    let target_sha = crate::diff::run_git_cmd(
        Command::new("git").args(["rev-parse", commit]),
    )?;
    let target_sha = target_sha.trim();

    let head_sha = crate::diff::run_git_cmd(
        Command::new("git").args(["rev-parse", "HEAD"]),
    )?;
    let head_sha = head_sha.trim();

    if target_sha == head_sha {
        // Simple case: amend HEAD
        let output = Command::new("git")
            .args(["commit", "--amend", "--no-edit"])
            .output()
            .context("failed to amend HEAD")?;
        if !output.status.success() {
            anyhow::bail!(
                "git commit --amend failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        // Get target commit subject for fixup message
        let subject = crate::diff::run_git_cmd(
            Command::new("git").args(["log", "-1", "--format=%s", target_sha]),
        )?;
        let subject = subject.trim();

        // Create fixup commit
        let output = Command::new("git")
            .args(["commit", "-m", &format!("fixup! {}", subject)])
            .output()
            .context("failed to create fixup commit")?;
        if !output.status.success() {
            anyhow::bail!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Check if target is root commit (has no parent)
        let is_root = Command::new("git")
            .args(["rev-parse", "--verify", &format!("{}^", target_sha)])
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(false);

        // Non-interactive autosquash rebase
        let mut rebase_cmd = Command::new("git");
        rebase_cmd.args(["rebase", "-i", "--autosquash", "--autostash"]);
        if is_root {
            rebase_cmd.arg("--root");
        } else {
            rebase_cmd.arg(&format!("{}~1", target_sha));
        }
        rebase_cmd.env("GIT_SEQUENCE_EDITOR", "true");

        let output = rebase_cmd.output().context("failed to run rebase")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "error: rebase conflict while fixing up {}",
                &target_sha[..7.min(target_sha.len())]
            );
            eprintln!("resolve conflicts and run: git rebase --continue");
            eprintln!("or abort with: git rebase --abort");
            anyhow::bail!("rebase failed: {}", stderr);
        }
    }

    // Print short sha + subject of the fixed-up commit
    let info = crate::diff::run_git_cmd(
        Command::new("git").args(["log", "-1", "--format=%h %s", target_sha]),
    );
    if let Ok(info) = info {
        eprintln!("fixed up {}", info.trim());
    }

    Ok(())
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
