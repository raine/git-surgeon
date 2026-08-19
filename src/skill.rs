use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::Path;

pub const SKILL_NAME: &str = "git-surgeon";
pub const SKILL_DESCRIPTION: &str = "Non-interactive hunk-level git staging, unstaging, discarding, undoing, fold, amend, squash, commit splitting, and commit reordering.";
pub const SKILL_SCHEMA_VERSION: u32 = 1;
pub const CLI_SCHEMA_VERSION: u32 = 1;
pub const SKILL_PATH_IN_REPO: &str = "skills/git-surgeon/SKILL.md";
pub const SKILL_CONTENT: &str = include_str!("../skills/git-surgeon/SKILL.md");
const MANAGED_PREFIX: &str = "<!-- Managed by git-surgeon skill install;";

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Platform {
    Claude,
    Pi,
    #[value(name = "opencode")]
    OpenCode,
    Codex,
    All,
}

impl Platform {
    fn relative_skill_path(self) -> Option<&'static str> {
        match self {
            Platform::Claude => Some(".claude/skills/git-surgeon/SKILL.md"),
            Platform::Pi => Some(".pi/agent/skills/git-surgeon/SKILL.md"),
            Platform::OpenCode => Some(".config/opencode/skills/git-surgeon/SKILL.md"),
            Platform::Codex => Some(".codex/skills/git-surgeon/SKILL.md"),
            Platform::All => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Platform::Claude => "claude",
            Platform::Pi => "pi",
            Platform::OpenCode => "opencode",
            Platform::Codex => "codex",
            Platform::All => "all",
        }
    }
}

const ALL_PLATFORMS: [Platform; 4] = [
    Platform::Claude,
    Platform::Pi,
    Platform::OpenCode,
    Platform::Codex,
];

#[derive(Debug, Serialize)]
pub struct SkillMetadata {
    pub name: &'static str,
    pub description: &'static str,
    pub cli_version: &'static str,
    pub schema_version: u32,
    pub path_in_repo: &'static str,
}

#[derive(Debug, Serialize)]
pub struct InstallResult {
    pub platform: &'static str,
    pub path: String,
    pub action: &'static str,
}

pub fn metadata() -> SkillMetadata {
    SkillMetadata {
        name: SKILL_NAME,
        description: SKILL_DESCRIPTION,
        cli_version: env!("CARGO_PKG_VERSION"),
        schema_version: SKILL_SCHEMA_VERSION,
        path_in_repo: SKILL_PATH_IN_REPO,
    }
}

pub fn validate_name(name: &str) -> Result<()> {
    if name != SKILL_NAME {
        bail!("unknown skill '{name}'; available skill: {SKILL_NAME}");
    }
    Ok(())
}

pub fn install(
    name: &str,
    platform: Platform,
    target_root: &Path,
    dry_run: bool,
    force: bool,
) -> Result<Vec<InstallResult>> {
    let platforms: &[Platform] = if matches!(platform, Platform::All) {
        &ALL_PLATFORMS
    } else {
        std::slice::from_ref(&platform)
    };
    install_many(name, platforms, target_root, dry_run, force)
}

pub fn install_many(
    name: &str,
    platforms: &[Platform],
    target_root: &Path,
    dry_run: bool,
    force: bool,
) -> Result<Vec<InstallResult>> {
    validate_name(name)?;

    // Validate every destination before making any changes.
    let mut plans = Vec::with_capacity(platforms.len());
    for &platform in platforms {
        let relative = platform
            .relative_skill_path()
            .expect("concrete platform has a path");
        let path = target_root.join(relative);
        let action = planned_action(&path, force)?;
        plans.push((platform, path, action));
    }

    if !dry_run {
        for (_, path, action) in &plans {
            if *action == "unchanged" {
                continue;
            }
            let parent = path.parent().expect("skill path has a parent");
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
            let mut temp = tempfile::NamedTempFile::new_in(parent).with_context(|| {
                format!("could not create temporary file in {}", parent.display())
            })?;
            temp.write_all(SKILL_CONTENT.as_bytes())?;
            temp.as_file().sync_all()?;
            temp.persist(path)
                .map_err(|error| error.error)
                .with_context(|| format!("could not install skill to {}", path.display()))?;
        }
    }

    Ok(plans
        .into_iter()
        .map(|(platform, path, action)| InstallResult {
            platform: platform.label(),
            path: path.display().to_string(),
            action: if dry_run {
                match action {
                    "installed" => "would_install",
                    "upgraded" => "would_upgrade",
                    "overwritten" => "would_overwrite",
                    _ => "unchanged",
                }
            } else {
                action
            },
        })
        .collect())
}

fn planned_action(path: &Path, force: bool) -> Result<&'static str> {
    if !path.exists() {
        return Ok("installed");
    }

    let existing = fs::read_to_string(path)
        .with_context(|| format!("could not read existing skill {}", path.display()))?;
    if existing.as_bytes() == SKILL_CONTENT.as_bytes() {
        return Ok("unchanged");
    }

    let Some(version) = managed_cli_version(&existing) else {
        if force {
            return Ok("overwritten");
        }
        bail!(
            "refusing to overwrite unmanaged skill {}; pass --force to replace it",
            path.display()
        );
    };

    match compare_versions(version, env!("CARGO_PKG_VERSION"))? {
        std::cmp::Ordering::Less => Ok("upgraded"),
        std::cmp::Ordering::Equal if force => Ok("overwritten"),
        std::cmp::Ordering::Equal => bail!(
            "refusing to overwrite modified managed skill {} at cli_version {}; pass --force to replace it",
            path.display(),
            version
        ),
        std::cmp::Ordering::Greater if force => Ok("overwritten"),
        std::cmp::Ordering::Greater => bail!(
            "refusing to overwrite newer managed skill {} (cli_version {} > {}); pass --force to replace it",
            path.display(),
            version,
            env!("CARGO_PKG_VERSION")
        ),
    }
}

fn managed_cli_version(content: &str) -> Option<&str> {
    let line = content
        .lines()
        .find(|line| line.starts_with(MANAGED_PREFIX))?;
    let value = line
        .split("cli_version=")
        .nth(1)?
        .split_whitespace()
        .next()?;
    Some(value.trim_end_matches(';'))
}

fn compare_versions(left: &str, right: &str) -> Result<std::cmp::Ordering> {
    fn parse(value: &str) -> Result<[u64; 3]> {
        let parts: Vec<_> = value.split('.').collect();
        if parts.len() != 3 {
            bail!("invalid managed skill cli_version '{value}'; expected MAJOR.MINOR.PATCH");
        }
        Ok([parts[0].parse()?, parts[1].parse()?, parts[2].parse()?])
    }
    Ok(parse(left)?.cmp(&parse(right)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_metadata_matches_frontmatter_and_marker() {
        assert!(SKILL_CONTENT.contains(&format!("cli_version: \"{}\"", env!("CARGO_PKG_VERSION"))));
        assert!(SKILL_CONTENT.contains("schema_version: 1"));
        assert!(SKILL_CONTENT.contains(&format!("cli_version={};", env!("CARGO_PKG_VERSION"))));
    }

    #[test]
    fn parses_and_compares_managed_versions() {
        assert_eq!(
            managed_cli_version(
                "<!-- Managed by git-surgeon skill install; cli_version=0.1.2; schema_version=1; do not edit. -->"
            ),
            Some("0.1.2")
        );
        assert_eq!(
            compare_versions("0.1.2", "0.1.17").unwrap(),
            std::cmp::Ordering::Less
        );
    }
}
