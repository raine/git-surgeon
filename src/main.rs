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
    /// Undo all changes to specific files from a commit
    UndoFile {
        /// File paths to undo
        files: Vec<String>,
        /// Commit to undo files from
        #[arg(long)]
        from: String,
    },
}

fn parse_line_range(s: &str) -> Result<(usize, usize), String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err("expected format: START-END (e.g. 5-30)".to_string());
    }
    let start: usize = parts[0].parse().map_err(|_| "invalid start number".to_string())?;
    let end: usize = parts[1].parse().map_err(|_| "invalid end number".to_string())?;
    if start == 0 || end == 0 || start > end {
        return Err("range must be 1-based and start <= end".to_string());
    }
    Ok((start, end))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hunks { staged, file, commit } => hunk::list_hunks(staged, file.as_deref(), commit.as_deref())?,
        Commands::Show { id, commit } => hunk::show_hunk(&id, commit.as_deref())?,
        Commands::Stage { ids, lines } => hunk::apply_hunks(&ids, hunk::ApplyMode::Stage, lines)?,
        Commands::Unstage { ids, lines } => hunk::apply_hunks(&ids, hunk::ApplyMode::Unstage, lines)?,
        Commands::Discard { ids, lines } => hunk::apply_hunks(&ids, hunk::ApplyMode::Discard, lines)?,
        Commands::Fixup { commit } => hunk::fixup(&commit)?,
        Commands::Undo { ids, from, lines } => hunk::undo_hunks(&ids, &from, lines)?,
        Commands::UndoFile { files, from } => hunk::undo_files(&files, &from)?,
    }

    Ok(())
}
