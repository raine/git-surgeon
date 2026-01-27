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
    },
    /// Show full diff for a specific hunk
    Show {
        /// Hunk ID
        id: String,
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Hunks { staged, file } => hunk::list_hunks(staged, file.as_deref())?,
        Commands::Show { id } => hunk::show_hunk(&id)?,
        Commands::Stage { ids } => hunk::apply_hunks(&ids, hunk::ApplyMode::Stage)?,
        Commands::Unstage { ids } => hunk::apply_hunks(&ids, hunk::ApplyMode::Unstage)?,
        Commands::Discard { ids } => hunk::apply_hunks(&ids, hunk::ApplyMode::Discard)?,
    }

    Ok(())
}
