use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// e.g. "src/main.rs"
    pub file: String,
    /// The full file header (--- a/... and +++ b/... lines)
    pub file_header: String,
    /// The @@ line, e.g. "@@ -12,4 +12,6 @@ fn main"
    pub header: String,
    /// All lines in the hunk (context, +, -)
    pub lines: Vec<String>,
}

pub fn run_git_diff(staged: bool, file: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(["diff", "--no-color", "--no-ext-diff", "--src-prefix=a/", "--dst-prefix=b/"]);
    if staged {
        cmd.arg("--cached");
    }
    if let Some(f) = file {
        cmd.arg("--").arg(f);
    }
    let output = cmd.output().context("failed to run git diff")?;
    if !output.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn parse_diff(input: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current_file = String::new();
    let mut current_file_header = String::new();
    let mut current_header: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in input.lines() {
        if line.starts_with("diff --git") {
            // Flush previous hunk
            if let Some(header) = current_header.take() {
                hunks.push(DiffHunk {
                    file: current_file.clone(),
                    file_header: current_file_header.clone(),
                    header,
                    lines: std::mem::take(&mut current_lines),
                });
            }
            current_file_header.clear();
        } else if line.starts_with("--- ") {
            current_file_header = line.to_string();
        } else if line.starts_with("+++ ") {
            current_file_header.push('\n');
            current_file_header.push_str(line);
            // Extract filename: "+++ b/src/main.rs" -> "src/main.rs"
            current_file = line
                .strip_prefix("+++ b/")
                .or_else(|| line.strip_prefix("+++ a/"))
                .unwrap_or(line)
                .to_string();
        } else if line.starts_with("@@ ") {
            // Flush previous hunk in same file
            if let Some(header) = current_header.take() {
                hunks.push(DiffHunk {
                    file: current_file.clone(),
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
            file: current_file,
            file_header: current_file_header,
            header,
            lines: current_lines,
        });
    }

    hunks
}
