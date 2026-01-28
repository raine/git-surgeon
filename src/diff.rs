use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// The primary file path for display/matching. Prefers the new-side path,
    /// but falls back to the old-side path for deletions (where new is /dev/null).
    pub file: String,
    /// Old-side file path (from `--- a/...`), or "/dev/null" for new files.
    pub old_file: String,
    /// New-side file path (from `+++ b/...`), or "/dev/null" for deleted files.
    pub new_file: String,
    /// The full file header (--- a/... and +++ b/... lines)
    pub file_header: String,
    /// The @@ line, e.g. "@@ -12,4 +12,6 @@ fn main"
    pub header: String,
    /// All lines in the hunk (context, +, -)
    pub lines: Vec<String>,
}

const DIFF_FORMAT_ARGS: &[&str] = &[
    "--no-color",
    "--no-ext-diff",
    "--src-prefix=a/",
    "--dst-prefix=b/",
];

pub fn run_git_diff(staged: bool, file: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.arg("diff");
    cmd.args(DIFF_FORMAT_ARGS);
    if staged {
        cmd.arg("--cached");
    }
    if let Some(f) = file {
        cmd.arg("--").arg(f);
    }
    run_git_cmd(&mut cmd)
}

pub fn run_git_diff_commit(commit: &str, file: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(["show", "--pretty="]);
    cmd.args(DIFF_FORMAT_ARGS);
    cmd.arg(commit);
    if let Some(f) = file {
        cmd.arg("--").arg(f);
    }
    run_git_cmd(&mut cmd)
}

pub fn run_git_cmd(cmd: &mut Command) -> Result<String> {
    let output = cmd.output().context("failed to run git command")?;
    if !output.status.success() {
        anyhow::bail!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Extract a file path from a `--- a/...` or `+++ b/...` line.
fn strip_diff_prefix(line: &str) -> &str {
    line.strip_prefix("--- a/")
        .or_else(|| line.strip_prefix("+++ b/"))
        .or_else(|| line.strip_prefix("--- /"))
        .or_else(|| line.strip_prefix("+++ /"))
        .or_else(|| line.strip_prefix("+++ a/"))
        .or_else(|| line.strip_prefix("--- "))
        .or_else(|| line.strip_prefix("+++ "))
        .unwrap_or(line)
}

pub fn parse_diff(input: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current_old_file = String::new();
    let mut current_new_file = String::new();
    let mut current_file_header = String::new();
    let mut current_header: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in input.lines() {
        if line.starts_with("diff --git") {
            // Flush previous hunk
            if let Some(header) = current_header.take() {
                hunks.push(DiffHunk {
                    file: display_file(&current_old_file, &current_new_file),
                    old_file: current_old_file.clone(),
                    new_file: current_new_file.clone(),
                    file_header: current_file_header.clone(),
                    header,
                    lines: std::mem::take(&mut current_lines),
                });
            }
            current_file_header.clear();
            current_old_file.clear();
            current_new_file.clear();
        } else if line.starts_with("--- ") {
            current_file_header = line.to_string();
            current_old_file = strip_diff_prefix(line).to_string();
        } else if line.starts_with("+++ ") {
            current_file_header.push('\n');
            current_file_header.push_str(line);
            current_new_file = strip_diff_prefix(line).to_string();
        } else if line.starts_with("@@ ") {
            // Flush previous hunk in same file
            if let Some(header) = current_header.take() {
                hunks.push(DiffHunk {
                    file: display_file(&current_old_file, &current_new_file),
                    old_file: current_old_file.clone(),
                    new_file: current_new_file.clone(),
                    file_header: current_file_header.clone(),
                    header,
                    lines: std::mem::take(&mut current_lines),
                });
            }
            current_header = Some(line.to_string());
        } else if current_header.is_some() {
            current_lines.push(line.to_string());
        }
    }

    // Flush last hunk
    if let Some(header) = current_header.take() {
        hunks.push(DiffHunk {
            file: display_file(&current_old_file, &current_new_file),
            old_file: current_old_file,
            new_file: current_new_file,
            file_header: current_file_header,
            header,
            lines: current_lines,
        });
    }

    hunks
}

/// Choose the display path for a hunk. Prefer new-side, fall back to old-side
/// for deletions where new is /dev/null.
fn display_file(old: &str, new: &str) -> String {
    if new == "dev/null" || new.is_empty() {
        old.to_string()
    } else {
        new.to_string()
    }
}
