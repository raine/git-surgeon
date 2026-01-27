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
    },
    /// Unstage hunks by ID
    Unstage {
        /// Hunk IDs to unstage
        ids: Vec<String>,
    },
    /// Discard working tree changes for hunks
    Discard {
        /// Hunk IDs to discard
        ids: Vec<String>,
    },
    /// Undo hunks from a commit, reverse-applying them to the working tree
    Undo {
        /// Hunk IDs to undo
        ids: Vec<String>,
        /// Commit to undo hunks from
        #[arg(long)]
        from: String,
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hunks { staged, file, commit } => hunk::list_hunks(staged, file.as_deref(), commit.as_deref())?,
        Commands::Show { id, commit } => hunk::show_hunk(&id, commit.as_deref())?,
        Commands::Stage { ids } => hunk::apply_hunks(&ids, hunk::ApplyMode::Stage)?,
        Commands::Unstage { ids } => hunk::apply_hunks(&ids, hunk::ApplyMode::Unstage)?,
        Commands::Discard { ids } => hunk::apply_hunks(&ids, hunk::ApplyMode::Discard)?,
        Commands::Fixup { commit } => hunk::fixup(&commit)?,
        Commands::Undo { ids, from } => hunk::undo_hunks(&ids, &from)?,
        Commands::UndoFile { files, from } => hunk::undo_files(&files, &from)?,
    }

    Ok(())
}
