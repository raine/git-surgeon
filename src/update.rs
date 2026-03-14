use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const REPO: &str = "raine/git-surgeon";
const BINARY_NAME: &str = "git-surgeon";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
const NOTIFY_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// Map OS/arch to the release artifact suffix used in GitHub releases.
fn platform_suffix() -> Result<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("darwin-arm64"),
        ("macos", "x86_64") => Ok("darwin-amd64"),
        ("linux", "x86_64") => Ok("linux-amd64"),
        ("linux", "aarch64") => Ok("linux-arm64"),
        (os, arch) => bail!("Unsupported platform: {os}/{arch}"),
    }
}

/// Check if the binary is managed by Homebrew.
fn is_homebrew_install(exe_path: &std::path::Path) -> bool {
    let path_str = exe_path.to_string_lossy();
    path_str.contains("/Cellar/") || path_str.contains("/homebrew/")
}

/// Fetch the latest release tag from GitHub API using curl.
fn fetch_latest_version() -> Result<String> {
    let output = Command::new("curl")
        .args([
            "-sSf",
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ])
        .output()
        .context("Failed to run curl. Is curl installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to fetch latest release: {}", stderr.trim());
    }

    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse GitHub API response")?;

    let tag = body["tag_name"]
        .as_str()
        .context("No tag_name in GitHub API response")?;

    Ok(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

/// Download a URL to a file path using curl.
fn download(url: &str, dest: &std::path::Path) -> Result<()> {
    let status = Command::new("curl")
        .args(["-sSLf", "-o"])
        .arg(dest)
        .arg(url)
        .status()
        .context("Failed to run curl")?;

    if !status.success() {
        bail!("Download failed: {url}");
    }
    Ok(())
}

/// Extract a tar.gz archive into a directory.
fn extract_tar(archive: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .status()
        .context("Failed to run tar")?;

    if !status.success() {
        bail!("Failed to extract archive");
    }
    Ok(())
}

/// Compute SHA-256 hash of a file using system tools.
fn sha256_of(path: &std::path::Path) -> Result<String> {
    // Try sha256sum first (common on Linux)
    if let Ok(output) = Command::new("sha256sum").arg(path).output()
        && output.status.success()
    {
        let out = String::from_utf8_lossy(&output.stdout);
        if let Some(hash) = out.split_whitespace().next() {
            return Ok(hash.to_string());
        }
    }

    // Fall back to shasum -a 256 (macOS)
    let output = Command::new("shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .context("Neither sha256sum nor shasum found. Cannot verify checksum.")?;

    if !output.status.success() {
        bail!("Checksum command failed");
    }

    let out = String::from_utf8_lossy(&output.stdout);
    out.split_whitespace()
        .next()
        .map(|s| s.to_string())
        .context("Could not parse checksum output")
}

/// Verify SHA-256 checksum of a file against the expected checksum line.
fn verify_checksum(file: &std::path::Path, expected_line: &str) -> Result<()> {
    // The .sha256 file format is: "<hash>  <filename>"
    let expected_hash = expected_line
        .split_whitespace()
        .next()
        .context("Invalid checksum file format")?;

    let actual_hash = sha256_of(file)?;
    if actual_hash != expected_hash {
        bail!("Checksum mismatch!\n  Expected: {expected_hash}\n  Got:      {actual_hash}");
    }
    Ok(())
}

/// Replace the current binary with the new one, with rollback on failure.
fn replace_binary(new_binary: &std::path::Path, current_exe: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let exe_dir = current_exe
        .parent()
        .context("Could not determine binary directory")?;

    // Copy to destination directory to avoid EXDEV (cross-device rename)
    let staged = exe_dir.join(format!(".{BINARY_NAME}.new"));
    std::fs::copy(new_binary, &staged).context("Failed to copy new binary to install directory")?;
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;

    // Rename current -> .old, then staged -> current
    let backup = exe_dir.join(format!(".{BINARY_NAME}.old"));
    std::fs::rename(current_exe, &backup).context("Failed to move current binary aside")?;

    if let Err(e) = std::fs::rename(&staged, current_exe) {
        // Rollback: restore the original
        let _ = std::fs::rename(&backup, current_exe);
        return Err(e).context("Failed to install new binary (rolled back)");
    }

    // Cleanup
    let _ = std::fs::remove_file(&backup);
    Ok(())
}

fn do_update(
    pb: &indicatif::ProgressBar,
    artifact_name: &str,
    current_exe: &std::path::Path,
) -> Result<String> {
    let latest_version = fetch_latest_version()?;

    if latest_version == CURRENT_VERSION {
        return Ok(format!("Already up to date (v{CURRENT_VERSION})"));
    }

    pb.set_message(format!("Downloading v{latest_version}..."));

    let tmp = tempfile::tempdir().context("Failed to create temp directory")?;
    let tar_path = tmp.path().join(format!("{artifact_name}.tar.gz"));
    let sha_path = tmp.path().join(format!("{artifact_name}.sha256"));

    let base_url = format!("https://github.com/{REPO}/releases/download/v{latest_version}");

    download(&format!("{base_url}/{artifact_name}.tar.gz"), &tar_path)?;
    download(&format!("{base_url}/{artifact_name}.sha256"), &sha_path)?;

    pb.set_message("Verifying checksum...");
    let sha_content = std::fs::read_to_string(&sha_path).context("Failed to read checksum file")?;
    verify_checksum(&tar_path, &sha_content)?;

    pb.set_message("Installing...");
    let extract_dir = tmp.path().join("extract");
    std::fs::create_dir(&extract_dir)?;
    extract_tar(&tar_path, &extract_dir)?;

    let new_binary = extract_dir.join(BINARY_NAME);
    if !new_binary.exists() {
        bail!("Extracted archive does not contain '{BINARY_NAME}' binary");
    }

    replace_binary(&new_binary, current_exe)?;

    Ok(format!(
        "Updated {BINARY_NAME} v{CURRENT_VERSION} -> v{latest_version}"
    ))
}

pub fn run() -> Result<()> {
    let current_exe =
        std::env::current_exe().context("Could not determine current executable path")?;

    // Guard: Homebrew-managed installs (canonicalize to resolve symlinks)
    let canonical_exe = std::fs::canonicalize(&current_exe).unwrap_or(current_exe.clone());
    if is_homebrew_install(&canonical_exe) {
        bail!("{BINARY_NAME} is managed by Homebrew. Run `brew upgrade {BINARY_NAME}` instead.");
    }

    let platform = platform_suffix()?;
    let artifact_name = format!("{BINARY_NAME}-{platform}");

    let pb = indicatif::ProgressBar::new_spinner();
    pb.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(120));
    pb.set_message("Checking for updates...");

    match do_update(&pb, &artifact_name, &current_exe) {
        Ok(msg) => {
            pb.finish_with_message(format!("✔ {msg}"));
            Ok(())
        }
        Err(e) => {
            pb.finish_with_message("✘ Update failed".to_string());
            Err(e)
        }
    }
}

// --- Auto-update check ---

#[derive(Debug, Serialize, Deserialize, Default)]
struct UpdateCache {
    latest_version: Option<String>,
    last_checked: Option<u64>,
    last_notified: Option<u64>,
}

fn update_cache_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join(".cache").join(BINARY_NAME);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("update_check.json"))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_cache(path: &std::path::Path) -> UpdateCache {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_cache(path: &std::path::Path, cache: &UpdateCache) {
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

/// Compare two version strings as numeric tuples (e.g. "0.1.10" > "0.1.9").
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> { v.split('.').filter_map(|s| s.parse().ok()).collect() };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

/// Called on CLI startup to show an update notice if one is cached.
/// Also spawns a background check if the cache is stale.
/// Designed to be completely non-blocking and fail-silent.
pub fn check_and_notify() {
    // Opt-out via environment variable
    if std::env::var("GIT_SURGEON_NO_UPDATE_CHECK").is_ok() {
        return;
    }

    // Don't print notices in non-interactive contexts
    use std::io::IsTerminal;
    if !std::io::stdout().is_terminal() || !std::io::stderr().is_terminal() {
        return;
    }

    let cache_path = match update_cache_path() {
        Some(p) => p,
        None => return,
    };

    let mut cache = load_cache(&cache_path);
    let now = now_secs();

    // Spawn background check if cache is stale
    if now.saturating_sub(cache.last_checked.unwrap_or(0)) > CHECK_INTERVAL_SECS {
        let spawned = std::env::current_exe().ok().and_then(|exe| {
            Command::new(exe)
                .arg("_check-update")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok()
        });

        // Only mark as checked if spawn succeeded to avoid silent blackout
        if spawned.is_some() {
            cache.last_checked = Some(now);
            save_cache(&cache_path, &cache);
        }
    }

    // Show notice if a newer version is available and we haven't notified recently
    if let Some(ref latest) = cache.latest_version
        && is_newer_version(latest, CURRENT_VERSION)
        && now.saturating_sub(cache.last_notified.unwrap_or(0)) > NOTIFY_INTERVAL_SECS
    {
        let is_brew = std::env::current_exe()
            .ok()
            .and_then(|p| std::fs::canonicalize(&p).ok())
            .is_some_and(|p| is_homebrew_install(&p));

        let update_cmd = if is_brew {
            format!("brew upgrade {BINARY_NAME}")
        } else {
            format!("{BINARY_NAME} update")
        };

        eprintln!(
            "Update available: {BINARY_NAME} v{CURRENT_VERSION} -> v{latest} (run `{update_cmd}`)"
        );

        cache.last_notified = Some(now);
        save_cache(&cache_path, &cache);
    }
}

/// Fetch latest version with a timeout (for background checks).
fn fetch_latest_version_with_timeout() -> Result<String> {
    let output = Command::new("curl")
        .args([
            "-sSf",
            "--connect-timeout",
            "5",
            "--max-time",
            "10",
            &format!("https://api.github.com/repos/{REPO}/releases/latest"),
        ])
        .output()
        .context("Failed to run curl")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to fetch latest release: {}", stderr.trim());
    }

    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse GitHub API response")?;

    let tag = body["tag_name"]
        .as_str()
        .context("No tag_name in GitHub API response")?;

    Ok(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

/// Hidden subcommand handler: fetch the latest version and update the cache.
pub fn run_background_check() -> Result<()> {
    let latest = fetch_latest_version_with_timeout()?;
    let now = now_secs();

    let cache_path = update_cache_path().context("Could not determine cache path")?;
    let mut cache = load_cache(&cache_path);

    // Reset notification timer when a new version is discovered
    if cache.latest_version.as_deref() != Some(&latest) {
        cache.last_notified = Some(0);
    }

    cache.latest_version = Some(latest);
    cache.last_checked = Some(now);
    save_cache(&cache_path, &cache);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_suffix_current() {
        let suffix = platform_suffix().unwrap();
        assert!(["darwin-arm64", "darwin-amd64", "linux-amd64", "linux-arm64"].contains(&suffix));
    }

    #[test]
    fn test_is_homebrew_cellar() {
        assert!(is_homebrew_install(std::path::Path::new(
            "/opt/homebrew/Cellar/git-surgeon/0.1.9/bin/git-surgeon"
        )));
    }

    #[test]
    fn test_is_homebrew_prefix() {
        assert!(is_homebrew_install(std::path::Path::new(
            "/usr/local/Cellar/git-surgeon/0.1.9/bin/git-surgeon"
        )));
    }

    #[test]
    fn test_is_not_homebrew_local_bin() {
        assert!(!is_homebrew_install(std::path::Path::new(
            "/usr/local/bin/git-surgeon"
        )));
    }

    #[test]
    fn test_is_not_homebrew_home() {
        assert!(!is_homebrew_install(std::path::Path::new(
            "/home/user/.local/bin/git-surgeon"
        )));
    }

    #[test]
    fn test_is_newer_version_patch() {
        assert!(is_newer_version("0.1.10", "0.1.9"));
    }

    #[test]
    fn test_is_newer_version_minor() {
        assert!(is_newer_version("0.2.0", "0.1.124"));
    }

    #[test]
    fn test_is_newer_version_major() {
        assert!(is_newer_version("1.0.0", "0.99.99"));
    }

    #[test]
    fn test_is_not_newer_same() {
        assert!(!is_newer_version("0.1.124", "0.1.124"));
    }

    #[test]
    fn test_is_not_newer_older() {
        assert!(!is_newer_version("0.1.9", "0.1.10"));
    }
}
