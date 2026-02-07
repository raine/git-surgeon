use anyhow::Result;
use clap::Parser;

mod blame;
mod diff;
mod hunk;
mod hunk_id;
mod patch;
mod skill;

#[derive(Parser)]
#[command(name = "git-surgeon")]
#[command(about = "Non-interactive hunk-level git staging for AI agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// List hunks in the diff
    Hunks {
        /// Show staged hunks (git diff --cached)
        #[arg(long)]
        staged: bool,
        /// Filter to a specific file
        #[arg(long)]
        file: Option<String>,
        /// Show hunks from a specific commit
        #[arg(long)]
        commit: Option<String>,
        /// Show full diff with line numbers (like show, but for all hunks)
        #[arg(long)]
        full: bool,
        /// Show git blame information for each line
        #[arg(long)]
        blame: bool,
    },
    /// Show full diff for a specific hunk
    Show {
        /// Hunk ID
        id: String,
        /// Look up hunk in a specific commit
        #[arg(long)]
        commit: Option<String>,
    },
    /// Stage hunks by ID
    Stage {
        /// Hunk IDs to stage
        ids: Vec<String>,
        /// Hunk-relative line range (e.g. 5-30) to apply only part of a hunk
        #[arg(long, value_parser = parse_line_range)]
        lines: Option<(usize, usize)>,
    },
    /// Unstage hunks by ID
    Unstage {
        /// Hunk IDs to unstage
        ids: Vec<String>,
        /// Hunk-relative line range (e.g. 5-30) to apply only part of a hunk
        #[arg(long, value_parser = parse_line_range)]
        lines: Option<(usize, usize)>,
    },
    /// Discard working tree changes for hunks
    Discard {
        /// Hunk IDs to discard
        ids: Vec<String>,
        /// Hunk-relative line range (e.g. 5-30) to apply only part of a hunk
        #[arg(long, value_parser = parse_line_range)]
        lines: Option<(usize, usize)>,
    },
    /// Undo hunks from a commit, reverse-applying them to the working tree
    Undo {
        /// Hunk IDs to undo
        ids: Vec<String>,
        /// Commit to undo hunks from
        #[arg(long)]
        from: String,
        /// Hunk-relative line range (e.g. 5-30) to apply only part of a hunk
        #[arg(long, value_parser = parse_line_range)]
        lines: Option<(usize, usize)>,
    },
    /// Fixup an earlier commit with currently staged changes
    Fixup {
        /// Target commit to fold staged changes into
        commit: String,
    },
    /// Change the commit message of an existing commit
    Reword {
        /// Target commit to reword
        commit: String,
        /// New commit message (multiple -m values are joined by blank lines)
        #[arg(short, long, required = true, num_args = 1)]
        message: Vec<String>,
    },
    /// Stage hunks and commit in one step
    Commit {
        /// Hunk IDs (optionally with :START-END range suffix)
        ids: Vec<String>,
        /// Commit message (multiple -m values are joined by blank lines, like git commit)
        #[arg(short, long, required = true, num_args = 1)]
        message: Vec<String>,
    },
    /// Undo all changes to specific files from a commit
    UndoFile {
        /// File paths to undo
        files: Vec<String>,
        /// Commit to undo files from
        #[arg(long)]
        from: String,
    },
    /// Split a commit into multiple commits by hunk selection
    #[command(disable_help_flag = false)]
    Split {
        /// Commit to split (e.g. HEAD, abc1234)
        commit: String,
        /// Remaining args: --pick <ids...> -m <msg> [-m <body>...] [--rest-message <msg>...]
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Squash commits from <commit>..HEAD into a single commit
    Squash {
        /// The oldest commit to include. All commits from here to HEAD are combined.
        commit: String,
        /// Commit message (required)
        #[arg(short, long, required = true, num_args = 1)]
        message: Vec<String>,
        /// Force squash even if range contains merge commits (which will be flattened)
        #[arg(long)]
        force: bool,
        /// Do not preserve the author from the oldest commit (use current user instead)
        #[arg(long)]
        no_preserve_author: bool,
    },
    /// Install the git-surgeon skill for AI coding assistants
    InstallSkill {
        /// Install for Claude Code (~/.claude/skills/)
        #[arg(long)]
        claude: bool,
        /// Install for OpenCode (~/.config/opencode/skills/)
        #[arg(long)]
        opencode: bool,
        /// Install for Codex (~/.codex/skills/)
        #[arg(long)]
        codex: bool,
    },
}

/// A group of hunk IDs (with optional line ranges) and a commit message.
pub struct PickGroup {
    pub ids: Vec<(String, Option<(usize, usize)>)>,
    pub message_parts: Vec<String>,
}

/// Parse the trailing args of the split command into pick groups and optional rest-message.
fn parse_split_args(args: &[String]) -> anyhow::Result<(Vec<PickGroup>, Option<Vec<String>>)> {
    let mut groups: Vec<PickGroup> = Vec::new();
    let mut rest_messages: Vec<String> = Vec::new();

    // State for the group currently being built
    let mut current_ids: Vec<(String, Option<(usize, usize)>)> = Vec::new();
    let mut current_msgs: Vec<String> = Vec::new();
    let mut seen_rest = false;

    // Helper to flush the current state into a PickGroup
    fn flush_group(
        groups: &mut Vec<PickGroup>,
        ids: &mut Vec<(String, Option<(usize, usize)>)>,
        msgs: &mut Vec<String>,
    ) -> anyhow::Result<()> {
        if !ids.is_empty() {
            if msgs.is_empty() {
                anyhow::bail!("--pick group missing --message");
            }
            groups.push(PickGroup {
                ids: std::mem::take(ids),
                message_parts: std::mem::take(msgs),
            });
        } else if !msgs.is_empty() {
            anyhow::bail!("--message without preceding --pick");
        }
        Ok(())
    }

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--pick" {
            if seen_rest {
                anyhow::bail!("--pick not allowed after --rest-message");
            }
            // Only flush if current group has messages (preserves backwards compat
            // with multiple --pick flags before --message)
            if !current_msgs.is_empty() {
                flush_group(&mut groups, &mut current_ids, &mut current_msgs)?;
            }

            i += 1;
            // Collect IDs until we hit a flag
            while i < args.len() && !args[i].starts_with('-') {
                let parsed = parse_pick_id(&args[i])?;
                current_ids.extend(parsed);
                i += 1;
            }
            if current_ids.is_empty() {
                anyhow::bail!("--pick requires at least one hunk ID");
            }
        } else if arg == "--message" || arg == "-m" {
            if seen_rest {
                anyhow::bail!("--message not allowed after --rest-message");
            }
            i += 1;
            if i >= args.len() {
                anyhow::bail!("--message requires a value");
            }
            if current_ids.is_empty() {
                anyhow::bail!("--message without preceding --pick");
            }
            current_msgs.push(args[i].clone());
            i += 1;
        } else if arg == "--rest-message" {
            // Flush any pending pick group first
            flush_group(&mut groups, &mut current_ids, &mut current_msgs)?;
            seen_rest = true;

            i += 1;
            if i >= args.len() {
                anyhow::bail!("--rest-message requires a value");
            }
            rest_messages.push(args[i].clone());
            i += 1;
        } else {
            anyhow::bail!("unexpected argument: {}", arg);
        }
    }

    // Flush the final group
    flush_group(&mut groups, &mut current_ids, &mut current_msgs)?;

    if groups.is_empty() {
        anyhow::bail!("at least one --pick ... --message pair is required");
    }

    let rest_message = if rest_messages.is_empty() {
        None
    } else {
        Some(rest_messages)
    };

    Ok((groups, rest_message))
}

///// Parse a pick ID that may have comma-separated ranges (e.g., "id:2,5-6,34").
/// Returns a list of (id, optional range) tuples - one per range, or one with None if no ranges.
#[allow(clippy::type_complexity)]
fn parse_pick_id(s: &str) -> anyhow::Result<Vec<(String, Option<(usize, usize)>)>> {
    if let Some((id, range_str)) = s.split_once(':') {
        let mut results = Vec::new();
        for part in range_str.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let range = parse_line_range(part).map_err(|e| anyhow::anyhow!(e))?;
            results.push((id.to_string(), Some(range)));
        }
        if results.is_empty() {
            // Edge case: "id:" with nothing after
            Ok(vec![(id.to_string(), None)])
        } else {
            Ok(results)
        }
    } else {
        Ok(vec![(s.to_string(), None)])
    }
}

fn parse_line_range(s: &str) -> Result<(usize, usize), String> {
    let (start, end) = if let Some((a, b)) = s.split_once('-') {
        let start: usize = a.parse().map_err(|_| "invalid start number".to_string())?;
        let end: usize = b.parse().map_err(|_| "invalid end number".to_string())?;
        (start, end)
    } else {
        let n: usize = s.parse().map_err(|_| "invalid line number".to_string())?;
        (n, n)
    };
    if start == 0 || end == 0 || start > end {
        return Err("range must be 1-based and start <= end".to_string());
    }
    Ok((start, end))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hunks {
            staged,
            file,
            commit,
            full,
            blame,
        } => hunk::list_hunks(staged, file.as_deref(), commit.as_deref(), full, blame)?,
        Commands::Show { id, commit } => hunk::show_hunk(&id, commit.as_deref())?,
        Commands::Stage { ids, lines } => hunk::apply_hunks(&ids, patch::ApplyMode::Stage, lines)?,
        Commands::Unstage { ids, lines } => {
            hunk::apply_hunks(&ids, patch::ApplyMode::Unstage, lines)?
        }
        Commands::Discard { ids, lines } => {
            hunk::apply_hunks(&ids, patch::ApplyMode::Discard, lines)?
        }
        Commands::Commit { ids, message } => hunk::commit_hunks(&ids, &message.join("\n\n"))?,
        Commands::Fixup { commit } => hunk::fixup(&commit)?,
        Commands::Reword { commit, message } => hunk::reword(&commit, &message.join("\n\n"))?,
        Commands::Undo { ids, from, lines } => hunk::undo_hunks(&ids, &from, lines)?,
        Commands::UndoFile { files, from } => hunk::undo_files(&files, &from)?,
        Commands::Split { commit, args } => {
            let (pick_groups, rest_message) = parse_split_args(&args)?;
            hunk::split(&commit, &pick_groups, rest_message.as_deref())?;
        }
        Commands::Squash {
            commit,
            message,
            force,
            no_preserve_author,
        } => {
            hunk::squash(&commit, &message.join("\n\n"), force, !no_preserve_author)?;
        }
        Commands::InstallSkill {
            claude,
            opencode,
            codex,
        } => {
            let mut platforms = Vec::new();
            if claude {
                platforms.push(skill::Platform::Claude);
            }
            if opencode {
                platforms.push(skill::Platform::OpenCode);
            }
            if codex {
                platforms.push(skill::Platform::Codex);
            }
            skill::install_skill(&platforms)?;
        }
    }

    Ok(())
}
