use anyhow::{Result, bail};
use std::fs;
use std::path::PathBuf;

const SKILL_CONTENT: &str = include_str!("../skills/git-surgeon/SKILL.md");

#[derive(Debug, Clone, Copy)]
pub enum Platform {
    Claude,
    OpenCode,
    Codex,
}

impl Platform {
    fn skill_dir(&self) -> PathBuf {
        let home = dirs::home_dir().expect("could not determine home directory");
        match self {
            Platform::Claude => home.join(".claude/skills/git-surgeon"),
            Platform::OpenCode => home.join(".config/opencode/skills/git-surgeon"),
            Platform::Codex => home.join(".codex/skills/git-surgeon"),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Platform::Claude => "Claude Code",
            Platform::OpenCode => "OpenCode",
            Platform::Codex => "Codex",
        }
    }
}

pub fn install_skill(platforms: &[Platform]) -> Result<()> {
    if platforms.is_empty() {
        bail!("at least one platform flag is required (--claude, --opencode, --codex)");
    }

    for platform in platforms {
        let dir = platform.skill_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join("SKILL.md");
        fs::write(&path, SKILL_CONTENT)?;
        println!("installed {} skill to {}", platform.name(), path.display());
    }

    Ok(())
}
