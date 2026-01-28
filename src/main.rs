use anyhow::Result;
use clap::Parser;

mod diff;
mod hunk;

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
        /// Remaining args parsed manually: --pick <ids...> --message <msg> [--rest-message <msg>]
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

/// A group of hunk IDs (with optional line ranges) and a commit message.
pub struct PickGroup {
    pub ids: Vec<(String, Option<(usize, usize)>)>,
    pub message: String,
}

/// Parse the trailing args of the split command into pick groups and optional rest-message.
fn parse_split_args(args: &[String]) -> anyhow::Result<(Vec<PickGroup>, Option<String>)> {
    let mut groups: Vec<PickGroup> = Vec::new();
    let mut rest_message: Option<String> = None;
    let mut i = 0;

    // Current state
    let mut current_ids: Vec<(String, Option<(usize, usize)>)> = Vec::new();

    while i < args.len() {
        let arg = &args[i];
        if arg == "--pick" {
            // If we already have IDs collected but no message yet, that's an error
            // (handled when we see the next --pick or --message)
            // Actually: flush is done when we see --message
            i += 1;
            // Collect IDs until next flag
            while i < args.len() && !args[i].starts_with("--") {
                let id_str = &args[i];
                let parsed = parse_pick_id(id_str)?;
                current_ids.push(parsed);
                i += 1;
            }
            if current_ids.is_empty() {
                anyhow::bail!("--pick requires at least one hunk ID");
            }
        } else if arg == "--message" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("--message requires a value");
            }
            if current_ids.is_empty() {
                anyhow::bail!("--message without preceding --pick");
            }
            groups.push(PickGroup {
                ids: std::mem::take(&mut current_ids),
                message: args[i].clone(),
            });
            i += 1;
        } else if arg == "--rest-message" {
            i += 1;
            if i >= args.len() {
                anyhow::bail!("--rest-message requires a value");
            }
            rest_message = Some(args[i].clone());
            i += 1;
        } else {
            anyhow::bail!("unexpected argument: {}", arg);
        }
    }

    if !current_ids.is_empty() {
        anyhow::bail!("--pick group missing --message");
    }
    if groups.is_empty() {
        anyhow::bail!("at least one --pick ... --message pair is required");
    }

    Ok((groups, rest_message))
}

fn parse_pick_id(s: &str) -> anyhow::Result<(String, Option<(usize, usize)>)> {
    if let Some((id, range)) = s.split_once(':') {
        let range = parse_line_range(range).map_err(|e| anyhow::anyhow!(e))?;
        Ok((id.to_string(), Some(range)))
    } else {
        Ok((s.to_string(), None))
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
        } => hunk::list_hunks(staged, file.as_deref(), commit.as_deref())?,
        Commands::Show { id, commit } => hunk::show_hunk(&id, commit.as_deref())?,
        Commands::Stage { ids, lines } => hunk::apply_hunks(&ids, hunk::ApplyMode::Stage, lines)?,
        Commands::Unstage { ids, lines } => {
            hunk::apply_hunks(&ids, hunk::ApplyMode::Unstage, lines)?
        }
        Commands::Discard { ids, lines } => {
            hunk::apply_hunks(&ids, hunk::ApplyMode::Discard, lines)?
        }
        Commands::Commit { ids, message } => hunk::commit_hunks(&ids, &message.join("\n\n"))?,
        Commands::Fixup { commit } => hunk::fixup(&commit)?,
        Commands::Undo { ids, from, lines } => hunk::undo_hunks(&ids, &from, lines)?,
        Commands::UndoFile { files, from } => hunk::undo_files(&files, &from)?,
        Commands::Split { commit, args } => {
            let (pick_groups, rest_message) = parse_split_args(&args)?;
            hunk::split(&commit, &pick_groups, rest_message.as_deref())?
        }
    }

    Ok(())
}
