use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::process::Command;

use crate::diff::DiffHunk;
use crate::hunk_id::assign_ids;
use crate::patch::{ApplyMode, apply_patch, build_patch, slice_hunk, slice_hunk_multi};

const MAX_PREVIEW_LINES: usize = 4;

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

        crate::diff::check_supported(hunk, id)?;

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

/// Parse an ID that may contain an inline range suffix (e.g. "a1b2c3d:1-11" or "a1b2c3d:5").
/// Returns (id, optional line range).
fn parse_id_range(raw: &str) -> Result<(&str, Option<(usize, usize)>)> {
    if let Some((id, range)) = raw.split_once(':') {
        let (start, end) = if let Some((a, b)) = range.split_once('-') {
            let start: usize = a
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid start number in '{}'", raw))?;
            let end: usize = b
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid end number in '{}'", raw))?;
            (start, end)
        } else {
            let n: usize = range
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid line number in '{}'", raw))?;
            (n, n)
        };
        if start == 0 || end == 0 || start > end {
            anyhow::bail!("range must be 1-based and start <= end in '{}'", raw);
        }
        Ok((id, Some((start, end))))
    } else {
        Ok((raw, None))
    }
}

/// Stage specified hunks and commit them. On commit failure, unstage to restore original state.
pub fn commit_hunks(ids: &[String], message: &str) -> Result<()> {
    // Refuse to proceed if there are already staged changes to avoid committing unrelated work
    let status = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .status()
        .context("failed to check staged changes")?;
    if !status.success() {
        anyhow::bail!("index already contains staged changes; commit or unstage them first");
    }

    let diff_output = crate::diff::run_git_diff(false, None)?;
    let hunks = crate::diff::parse_diff(&diff_output);
    let identified = assign_ids(&hunks);

    // Build patch from all requested hunks, grouping ranges by hunk ID
    let mut hunk_ranges: Vec<(String, Vec<(usize, usize)>)> = Vec::new();
    for raw_id in ids {
        let (id, lines) = parse_id_range(raw_id)?;
        if let Some(entry) = hunk_ranges.iter_mut().find(|(eid, _)| eid == id) {
            if let Some(range) = lines {
                entry.1.push(range);
            }
        } else {
            let ranges = match lines {
                Some(range) => vec![range],
                None => vec![],
            };
            hunk_ranges.push((id.to_string(), ranges));
        }
    }

    let mut combined_patch = String::new();
    for (id, ranges) in &hunk_ranges {
        let (_, hunk) = identified
            .iter()
            .find(|(hunk_id, _)| hunk_id == id)
            .ok_or_else(|| anyhow::anyhow!("hunk {} not found (re-run 'hunks')", id))?;

        crate::diff::check_supported(hunk, id)?;

        let patched_hunk = if ranges.is_empty() {
            (*hunk).clone()
        } else {
            slice_hunk_multi(hunk, ranges, false)?
        };
        combined_patch.push_str(&build_patch(&patched_hunk));
        eprintln!("{}", id);
    }

    // Stage the hunks
    apply_patch(&combined_patch, &ApplyMode::Stage)?;

    // Commit
    let output = Command::new("git")
        .args(["commit", "-m", message])
        .output()
        .context("failed to run git commit")?;

    if !output.status.success() {
        // Unstage to restore original state
        let _ = apply_patch(&combined_patch, &ApplyMode::Unstage);
        anyhow::bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

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

        crate::diff::check_supported(hunk, id)?;

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
    let mut matched_files = HashSet::new();
    for hunk in &hunks {
        if files
            .iter()
            .any(|f| f == &hunk.file || f == &hunk.old_file || f == &hunk.new_file)
        {
            crate::diff::check_supported(hunk, &hunk.file)?;
            combined_patch.push_str(&build_patch(hunk));
            matched_files.extend(
                files
                    .iter()
                    .filter(|f| *f == &hunk.file || *f == &hunk.old_file || *f == &hunk.new_file),
            );
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

/// Fold currently staged changes into an earlier commit via autosquash rebase.
/// If the target is HEAD, uses simple --amend instead.
pub fn fixup(commit: &str) -> Result<()> {
    // Verify there are staged changes
    let status = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .status()
        .context("failed to run git diff")?;
    if status.success() {
        anyhow::bail!("no staged changes to fixup");
    }

    // Check no rebase/cherry-pick in progress
    check_no_rebase_in_progress()?;

    // Resolve the target commit SHA
    let target_sha = crate::diff::run_git_cmd(Command::new("git").args(["rev-parse", commit]))?;
    let target_sha = target_sha.trim();

    let head_sha = crate::diff::run_git_cmd(Command::new("git").args(["rev-parse", "HEAD"]))?;
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
        let subject = crate::diff::run_git_cmd(Command::new("git").args([
            "log",
            "-1",
            "--format=%s",
            target_sha,
        ]))?;
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
            rebase_cmd.arg(format!("{}~1", target_sha));
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
    let info = crate::diff::run_git_cmd(Command::new("git").args([
        "log",
        "-1",
        "--format=%h %s",
        target_sha,
    ]));
    if let Ok(info) = info {
        eprintln!("fixed up {}", info.trim());
    }

    Ok(())
}

/// Split a commit into multiple commits by hunk selection.
pub fn split(
    commit: &str,
    pick_groups: &[crate::PickGroup],
    rest_message: Option<&[String]>,
) -> Result<()> {
    // Check working tree is clean
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .context("failed to check git status")?;
    if !String::from_utf8_lossy(&status.stdout).trim().is_empty() {
        anyhow::bail!("working tree is dirty; commit or stash changes before splitting");
    }

    check_no_rebase_in_progress()?;

    // Resolve target commit
    let target_sha = crate::diff::run_git_cmd(Command::new("git").args(["rev-parse", commit]))?;
    let target_sha = target_sha.trim().to_string();

    let head_sha = crate::diff::run_git_cmd(Command::new("git").args(["rev-parse", "HEAD"]))?;
    let head_sha = head_sha.trim().to_string();

    let is_head = target_sha == head_sha;

    // Get hunks from the target commit and validate all pick IDs exist
    let diff_output = crate::diff::run_git_diff_commit(&target_sha, None)?;
    let hunks = crate::diff::parse_diff(&diff_output);
    let identified = assign_ids(&hunks);

    // Validate all referenced IDs exist and are supported
    for group in pick_groups {
        for (id, _) in &group.ids {
            let (_, hunk) = identified
                .iter()
                .find(|(hid, _)| hid == id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "hunk {} not found in commit {}",
                        id,
                        &target_sha[..7.min(target_sha.len())]
                    )
                })?;
            crate::diff::check_supported(hunk, id)?;
        }
    }

    // Get original commit message for rest-message default
    let original_message = crate::diff::run_git_cmd(Command::new("git").args([
        "log",
        "-1",
        "--format=%B",
        &target_sha,
    ]))?;
    let original_message = original_message.trim();
    let rest_msg_joined;
    let rest_msg = match rest_message {
        Some(parts) => {
            rest_msg_joined = parts.join("\n\n");
            rest_msg_joined.as_str()
        }
        None => original_message,
    };

    // Collect all picked IDs to determine "rest"
    let mut all_picked: HashSet<String> = HashSet::new();
    for group in pick_groups {
        for (id, _) in &group.ids {
            all_picked.insert(id.clone());
        }
    }

    if !is_head {
        start_rebase_at_commit(&target_sha)?;
    } else {
        // HEAD: just reset
        let output = Command::new("git")
            .args(["reset", "HEAD~"])
            .output()
            .context("failed to reset HEAD")?;
        if !output.status.success() {
            anyhow::bail!(
                "git reset failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // Now changes are in the working tree. Stage and commit each pick group.

    // Map original ID -> file for fallback when hunk ID changes after partial apply
    let initial_diff = crate::diff::run_git_diff(false, None)?;
    let initial_hunks = crate::diff::parse_diff(&initial_diff);
    let initial_identified = assign_ids(&initial_hunks);
    let id_to_file: HashMap<String, String> = initial_identified
        .iter()
        .map(|(id, hunk)| (id.clone(), hunk.file.clone()))
        .collect();

    for group in pick_groups {
        // Re-read current diff for fresh context that matches the index
        let diff_output = crate::diff::run_git_diff(false, None)?;
        let current_hunks = crate::diff::parse_diff(&diff_output);
        let current_identified = assign_ids(&current_hunks);

        let mut combined_patch = String::new();

        // Group line ranges by hunk ID so same-hunk entries produce one patch
        let mut hunk_ranges: Vec<(String, Vec<(usize, usize)>)> = Vec::new();
        for (id, lines_range) in &group.ids {
            if let Some(entry) = hunk_ranges.iter_mut().find(|(eid, _)| eid == id) {
                if let Some(range) = lines_range {
                    entry.1.push(*range);
                }
            } else {
                let ranges = match lines_range {
                    Some(range) => vec![*range],
                    None => vec![],
                };
                hunk_ranges.push((id.clone(), ranges));
            }
        }

        for (id, ranges) in &hunk_ranges {
            // Find hunk by ID to handle multiple hunks in the same file correctly.
            // Fall back to file match for line-range scenarios where the hunk ID
            // changes after partial apply.
            let hunk = current_identified
                .iter()
                .find(|(hunk_id, _)| hunk_id == id)
                .map(|(_, h)| *h)
                .or_else(|| {
                    id_to_file
                        .get(id)
                        .and_then(|file| current_hunks.iter().find(|h| &h.file == file))
                })
                .ok_or_else(|| anyhow::anyhow!("hunk {} not found in unstaged changes", id))?;

            let patched_hunk = if ranges.is_empty() {
                hunk.clone()
            } else {
                slice_hunk_multi(hunk, ranges, false)?
            };
            combined_patch.push_str(&build_patch(&patched_hunk));
        }

        apply_patch(&combined_patch, &ApplyMode::Stage)?;

        // Commit
        let message = group.message_parts.join("\n\n");
        let output = Command::new("git")
            .args(["commit", "-m", &message])
            .output()
            .context("failed to commit")?;
        if !output.status.success() {
            anyhow::bail!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Print only the subject line
        let subject = message.lines().next().unwrap_or(&message);
        eprintln!("committed: {}", subject);
    }

    // Stage and commit remaining changes (if any)
    let remaining_diff = crate::diff::run_git_diff(false, None)?;
    if !remaining_diff.trim().is_empty() {
        let remaining_hunks = crate::diff::parse_diff(&remaining_diff);
        let mut combined_patch = String::new();
        for hunk in &remaining_hunks {
            combined_patch.push_str(&build_patch(hunk));
        }
        apply_patch(&combined_patch, &ApplyMode::Stage)?;

        let output = Command::new("git")
            .args(["commit", "-m", rest_msg])
            .output()
            .context("failed to commit remaining")?;
        if !output.status.success() {
            anyhow::bail!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Print only the subject line
        let subject = rest_msg.lines().next().unwrap_or(rest_msg);
        eprintln!("committed: {}", subject);
    }

    // Continue rebase if non-HEAD
    if !is_head {
        let output = Command::new("git")
            .args(["rebase", "--continue"])
            .output()
            .context("failed to continue rebase")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("error: rebase continue failed");
            eprintln!("resolve conflicts and run: git rebase --continue");
            eprintln!("or abort with: git rebase --abort");
            anyhow::bail!("rebase continue failed: {}", stderr);
        }
    }

    Ok(())
}

fn check_no_rebase_in_progress() -> Result<()> {
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
    Ok(())
}

fn start_rebase_at_commit(target_sha: &str) -> Result<()> {
    let is_root = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{}^", target_sha)])
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(false);

    // We need a custom sequence editor that marks the target commit as "edit"
    let short_sha = &target_sha[..7.min(target_sha.len())];
    // Use sed to change "pick <sha>" to "edit <sha>" for the target commit
    let sed_script = format!("s/^pick {} /edit {} /", short_sha, short_sha);

    let mut rebase_cmd = Command::new("git");
    rebase_cmd.args(["rebase", "-i", "--autostash"]);
    if is_root {
        rebase_cmd.arg("--root");
    } else {
        rebase_cmd.arg(format!("{}~1", target_sha));
    }
    rebase_cmd.env(
        "GIT_SEQUENCE_EDITOR",
        format!("sed -i.bak '{}'", sed_script),
    );

    let output = rebase_cmd.output().context("failed to start rebase")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("rebase failed: {}", stderr);
    }

    // Now we should be paused at the target commit. Reset it.
    let output = Command::new("git")
        .args(["reset", "HEAD~"])
        .output()
        .context("failed to reset commit")?;
    if !output.status.success() {
        anyhow::bail!(
            "git reset failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
