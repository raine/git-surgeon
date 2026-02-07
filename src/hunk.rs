use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::process::Command;

use crate::diff::DiffHunk;
use crate::hunk_id::assign_ids;
use crate::patch::{
    ApplyMode, apply_patch, build_patch, slice_hunk, slice_hunk_multi, slice_hunk_with_state,
};

const MAX_PREVIEW_LINES: usize = 4;

pub fn list_hunks(
    staged: bool,
    file: Option<&str>,
    commit: Option<&str>,
    full: bool,
    blame: bool,
) -> Result<()> {
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

        if blame {
            // Blame mode: show all lines with blame hashes (takes precedence over full)
            print_blamed_lines(hunk, commit)?;
        } else if full {
            // Full mode: show all lines with line numbers (like show command)
            let width = hunk.lines.len().to_string().len();
            for (i, line) in hunk.lines.iter().enumerate() {
                println!("{:>w$}:{}", i + 1, line, w = width);
            }
        } else {
            // Preview mode: show up to MAX_PREVIEW_LINES changed lines
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
        }
        println!();
    }

    Ok(())
}

fn print_blamed_lines(hunk: &crate::diff::DiffHunk, commit: Option<&str>) -> Result<()> {
    use crate::blame::{get_blame, parse_hunk_header};

    let (old_from, old_count, new_from, new_count) =
        parse_hunk_header(&hunk.header).unwrap_or((1, 0, 1, 0));

    // Determine blame revisions based on diff type
    // For commit diffs: old = commit^, new = commit
    // For unstaged/staged: old = HEAD, new = working tree (returns 0000000)
    let (old_rev_str, new_rev): (String, Option<&str>) = match commit {
        Some(c) => (format!("{}^", c), Some(c)),
        None => ("HEAD".to_string(), None),
    };

    // Get blame for old side (for context and removed lines)
    let old_blame = if hunk.old_file != "dev/null" && old_count > 0 {
        get_blame(&hunk.old_file, old_from, old_count, Some(&old_rev_str)).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Get blame for new side (for context and added lines)
    let new_blame = if hunk.new_file != "dev/null" && new_count > 0 {
        get_blame(&hunk.new_file, new_from, new_count, new_rev).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Walk through lines with indices
    let mut old_idx = 0usize;
    let mut new_idx = 0usize;

    for line in &hunk.lines {
        let hash = if line.starts_with(' ') {
            // Context line: use new side blame (exists in both)
            let h = new_blame
                .get(new_idx)
                .map(|s| s.as_str())
                .unwrap_or("0000000");
            old_idx += 1;
            new_idx += 1;
            h.to_string()
        } else if line.starts_with('-') {
            // Removed line: use old side blame
            let h = old_blame
                .get(old_idx)
                .map(|s| s.as_str())
                .unwrap_or("0000000");
            old_idx += 1;
            h.to_string()
        } else if line.starts_with('+') {
            // Added line: use new side blame (0000000 for uncommitted)
            let h = new_blame
                .get(new_idx)
                .map(|s| s.as_str())
                .unwrap_or("0000000");
            new_idx += 1;
            h.to_string()
        } else {
            // Unknown line type (e.g., "\ No newline"), skip blame
            println!("  {}", line);
            continue;
        };

        // Keep indentation to match existing preview line style
        println!("  {} {}", hash, line);
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

/// Parse an ID that may contain inline range suffixes.
/// Supports: "id", "id:5", "id:1-11", "id:2,5-6,34" (comma-separated).
/// Returns (id, vector of ranges). Empty vector means "whole hunk".
fn parse_id_range(raw: &str) -> Result<(&str, Vec<(usize, usize)>)> {
    if let Some((id, range_str)) = raw.split_once(':') {
        let mut ranges = Vec::new();
        for part in range_str.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (start, end) = if let Some((a, b)) = part.split_once('-') {
                let start: usize = a
                    .parse()
                    .map_err(|_| anyhow::anyhow!("invalid start number in '{}'", raw))?;
                let end: usize = b
                    .parse()
                    .map_err(|_| anyhow::anyhow!("invalid end number in '{}'", raw))?;
                (start, end)
            } else {
                let n: usize = part
                    .parse()
                    .map_err(|_| anyhow::anyhow!("invalid line number in '{}'", raw))?;
                (n, n)
            };
            if start == 0 || end == 0 || start > end {
                anyhow::bail!("range must be 1-based and start <= end in '{}'", raw);
            }
            ranges.push((start, end));
        }
        Ok((id, ranges))
    } else {
        Ok((raw, Vec::new()))
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
        let (id, ranges) = parse_id_range(raw_id)?;
        if let Some(entry) = hunk_ranges.iter_mut().find(|(eid, _)| eid == id) {
            entry.1.extend(ranges);
        } else {
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

/// Change the commit message of an existing commit.
pub fn reword(commit: &str, message: &str) -> Result<()> {
    // Check no rebase/cherry-pick in progress
    check_no_rebase_in_progress()?;

    // Resolve the target commit SHA
    let target_sha = crate::diff::run_git_cmd(Command::new("git").args(["rev-parse", commit]))?;
    let target_sha = target_sha.trim();

    let head_sha = crate::diff::run_git_cmd(Command::new("git").args(["rev-parse", "HEAD"]))?;
    let head_sha = head_sha.trim();

    // Track distance from target to HEAD for later (used to find new SHA after rebase)
    let distance = crate::diff::run_git_cmd(Command::new("git").args([
        "rev-list",
        "--count",
        &format!("{}..HEAD", target_sha),
    ]))?;
    let distance: usize = distance.trim().parse().unwrap_or(0);

    if target_sha == head_sha {
        // Simple case: amend HEAD with new message
        let output = Command::new("git")
            .args(["commit", "--amend", "-m", message])
            .output()
            .context("failed to amend HEAD")?;
        if !output.status.success() {
            anyhow::bail!(
                "git commit --amend failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        // Get original commit subject for reword marker
        let subject = crate::diff::run_git_cmd(Command::new("git").args([
            "log",
            "-1",
            "--format=%s",
            target_sha,
        ]))?;
        let subject = subject.trim();

        // Create empty reword commit with new message
        let output = Command::new("git")
            .args([
                "commit",
                "--allow-empty",
                "-m",
                &format!("amend! {}\n\n{}", subject, message),
            ])
            .output()
            .context("failed to create reword commit")?;
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
                "error: rebase conflict while rewording {}",
                &target_sha[..7.min(target_sha.len())]
            );
            eprintln!("resolve conflicts and run: git rebase --continue");
            eprintln!("or abort with: git rebase --abort");
            anyhow::bail!("rebase failed: {}", stderr);
        }
    }

    // Print short sha + new subject of the reworded commit
    // Use HEAD~distance to find the commit at the same position after rebase
    let ref_spec = if distance == 0 {
        "HEAD".to_string()
    } else {
        format!("HEAD~{}", distance)
    };
    let info = crate::diff::run_git_cmd(Command::new("git").args([
        "log",
        "-1",
        "--format=%h %s",
        &ref_spec,
    ]));
    if let Ok(info) = info {
        eprintln!("reworded {}", info.trim());
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

    // Build stateful hunk tracking: original hunks with picked state
    // This keeps line ranges stable (always relative to original commit)
    struct HunkState {
        hunk: DiffHunk,
        picked: Vec<bool>, // which lines have been picked in previous groups
    }

    let mut hunk_states: HashMap<String, HunkState> = identified
        .iter()
        .map(|(id, hunk)| {
            (
                id.clone(),
                HunkState {
                    hunk: (*hunk).clone(),
                    picked: vec![false; hunk.lines.len()],
                },
            )
        })
        .collect();

    // Pre-validate all line ranges before modifying git state
    for group in pick_groups {
        // Group line ranges by hunk ID
        let mut hunk_ranges: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
        for (id, lines_range) in &group.ids {
            if let Some(range) = lines_range {
                hunk_ranges.entry(id.clone()).or_default().push(*range);
            }
        }

        for (id, ranges) in &hunk_ranges {
            let state = hunk_states
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("hunk {} not found", id))?;

            for (start, end) in ranges {
                if *start == 0 || *end == 0 {
                    anyhow::bail!("line ranges are 1-based, got {}:{}-{}", id, start, end);
                }
                if *end > state.hunk.lines.len() {
                    anyhow::bail!(
                        "line range {}:{}-{} exceeds hunk length ({})",
                        id,
                        start,
                        end,
                        state.hunk.lines.len()
                    );
                }
            }
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

    // Now changes are in the working tree. Stage and commit each pick group
    // using the stateful approach (line ranges always relative to original commit).

    for group in pick_groups {
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
            let state = hunk_states
                .get_mut(id)
                .ok_or_else(|| anyhow::anyhow!("hunk {} not found", id))?;

            // Build selection mask for this group
            let mut selected = vec![false; state.hunk.lines.len()];

            if ranges.is_empty() {
                // No line ranges: select all remaining change lines
                for (i, line) in state.hunk.lines.iter().enumerate() {
                    if (line.starts_with('+') || line.starts_with('-')) && !state.picked[i] {
                        selected[i] = true;
                    }
                }
            } else {
                // Select lines in specified ranges
                for (start, end) in ranges {
                    #[allow(clippy::needless_range_loop)]
                    for i in (*start - 1)..*end {
                        if i < state.hunk.lines.len() {
                            let line = &state.hunk.lines[i];
                            // Only select change lines, not context
                            if line.starts_with('+') || line.starts_with('-') {
                                if state.picked[i] {
                                    anyhow::bail!(
                                        "line {} in hunk {} was already picked in a previous group",
                                        i + 1,
                                        id
                                    );
                                }
                                selected[i] = true;
                            }
                        }
                    }
                }
            }

            // Check we're actually selecting something
            let has_changes = selected.iter().any(|&s| s);
            if !has_changes {
                // Skip this hunk if nothing to select
                continue;
            }

            // Build patch using stateful slicing
            let patched_hunk = slice_hunk_with_state(&state.hunk, &state.picked, &selected)?;
            combined_patch.push_str(&build_patch(&patched_hunk));

            // Mark selected lines as picked for next groups
            for (i, sel) in selected.iter().enumerate() {
                if *sel {
                    state.picked[i] = true;
                }
            }
        }

        if combined_patch.is_empty() {
            anyhow::bail!("no changes selected for commit");
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
    // Build patches for all unpicked change lines
    let mut has_remaining = false;
    let mut combined_patch = String::new();

    for (id, state) in &hunk_states {
        // Check if any change lines remain unpicked
        let mut remaining_selected = vec![false; state.hunk.lines.len()];
        for (i, line) in state.hunk.lines.iter().enumerate() {
            if (line.starts_with('+') || line.starts_with('-')) && !state.picked[i] {
                remaining_selected[i] = true;
                has_remaining = true;
            }
        }

        if remaining_selected.iter().any(|&s| s) {
            let patched_hunk =
                slice_hunk_with_state(&state.hunk, &state.picked, &remaining_selected)?;
            combined_patch.push_str(&build_patch(&patched_hunk));

            // Mark as picked (for consistency, though we're done)
            for (i, sel) in remaining_selected.iter().enumerate() {
                if *sel {
                    // We'd update state.picked here but we're borrowing immutably
                    let _ = (id, sel, i); // suppress unused warnings
                }
            }
        }
    }

    if has_remaining {
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

/// Squash commits from <commit>..HEAD into a single commit.
pub fn squash(commit: &str, message: &str, force: bool, preserve_author: bool) -> Result<()> {
    check_no_rebase_in_progress()?;

    // Autostash if working tree is dirty (tracked files only)
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .context("failed to check git status")?;
    let needs_stash = !String::from_utf8_lossy(&status.stdout).trim().is_empty();

    if needs_stash {
        let output = Command::new("git")
            .args(["stash", "push", "-m", "git-surgeon squash autostash"])
            .output()
            .context("failed to stash changes")?;
        if !output.status.success() {
            anyhow::bail!(
                "git stash failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // Resolve target commit SHA
    let target_sha = crate::diff::run_git_cmd(Command::new("git").args(["rev-parse", commit]))
        .with_context(|| format!("could not resolve commit '{}'", commit))?;
    let target_sha = target_sha.trim();

    let head_sha = crate::diff::run_git_cmd(Command::new("git").args(["rev-parse", "HEAD"]))?;
    let head_sha = head_sha.trim();

    if target_sha == head_sha {
        anyhow::bail!("nothing to squash: target commit is HEAD");
    }

    // Extract author and date from target commit if preserving
    let (author, author_date) = if preserve_author {
        let ident = crate::diff::run_git_cmd(Command::new("git").args([
            "log",
            "-1",
            "--format=%an <%ae>",
            target_sha,
        ]))?
        .trim()
        .to_string();

        let date = crate::diff::run_git_cmd(Command::new("git").args([
            "log",
            "-1",
            "--format=%aI", // ISO 8601 format for unambiguous parsing
            target_sha,
        ]))?
        .trim()
        .to_string();

        (Some(ident), Some(date))
    } else {
        (None, None)
    };

    // Verify target is ancestor of HEAD
    let is_ancestor = Command::new("git")
        .args(["merge-base", "--is-ancestor", target_sha, "HEAD"])
        .status()
        .context("failed to check ancestry")?;
    if !is_ancestor.success() {
        anyhow::bail!(
            "commit {} is not an ancestor of HEAD",
            &target_sha[..7.min(target_sha.len())]
        );
    }

    // Check for merge commits in range (they will be flattened)
    if !force {
        let merges = Command::new("git")
            .args(["rev-list", "--merges", &format!("{}..HEAD", target_sha)])
            .output()
            .context("failed to check for merge commits")?;
        if !String::from_utf8_lossy(&merges.stdout).trim().is_empty() {
            anyhow::bail!(
                "range contains merge commits which will be flattened; use --force to proceed"
            );
        }
    }

    // Check if target is root commit
    let is_root = Command::new("git")
        .args(["rev-parse", "--verify", &format!("{}^", target_sha)])
        .output()
        .map(|o| !o.status.success())
        .unwrap_or(false);

    if is_root {
        // For root commit: delete HEAD ref to create orphan state, then commit
        // This preserves hooks and GPG signing (unlike commit-tree)
        let output = Command::new("git")
            .args(["update-ref", "-d", "HEAD"])
            .output()
            .context("failed to delete HEAD ref")?;
        if !output.status.success() {
            anyhow::bail!(
                "git update-ref failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Commit (git treats this as the first commit)
        let mut commit_cmd = Command::new("git");
        commit_cmd.args(["commit", "-m", message]);
        if let Some(ref auth) = author {
            commit_cmd.args(["--author", auth]);
        }
        if let Some(ref date) = author_date {
            commit_cmd.args(["--date", date]);
        }
        let output = commit_cmd.output().context("failed to commit")?;
        if !output.status.success() {
            anyhow::bail!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        // Normal case: reset to parent of target
        let output = Command::new("git")
            .args(["reset", "--soft", &format!("{}^", target_sha)])
            .output()
            .context("failed to reset")?;
        if !output.status.success() {
            anyhow::bail!(
                "git reset failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Commit with new message
        let mut commit_cmd = Command::new("git");
        commit_cmd.args(["commit", "-m", message]);
        if let Some(ref auth) = author {
            commit_cmd.args(["--author", auth]);
        }
        if let Some(ref date) = author_date {
            commit_cmd.args(["--date", date]);
        }
        let output = commit_cmd.output().context("failed to commit")?;
        if !output.status.success() {
            anyhow::bail!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // Count how many commits were squashed
    let count = crate::diff::run_git_cmd(Command::new("git").args([
        "rev-list",
        "--count",
        &format!("{}..{}", target_sha, head_sha),
    ]))?;
    let count: i32 = count.trim().parse().unwrap_or(0);

    eprintln!("squashed {} commits", count + 1);

    // Restore stashed changes
    if needs_stash {
        let output = Command::new("git")
            .args(["stash", "pop"])
            .output()
            .context("failed to pop stash")?;
        if !output.status.success() {
            eprintln!(
                "warning: stash pop failed (conflicts?), run 'git stash pop' manually: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
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
