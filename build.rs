use std::path::PathBuf;
use std::process::Command;

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-env-changed=GIT_SURGEON_GIT_COMMIT");
    if let Ok(commit) = std::env::var("GIT_SURGEON_GIT_COMMIT") {
        assert!(
            commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "GIT_SURGEON_GIT_COMMIT must be a full 40-character hexadecimal SHA"
        );
        println!("cargo:rustc-env=GIT_SURGEON_GIT_COMMIT={commit}");
        return;
    }

    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let Some(git_root) = git_output(&["rev-parse", "--show-toplevel"]) else {
        return;
    };
    if PathBuf::from(git_root).canonicalize().ok() != manifest_dir.canonicalize().ok() {
        return;
    }

    if let Some(head_path) = git_output(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head_path}");
    }
    if let Some(head_ref) = git_output(&["symbolic-ref", "-q", "HEAD"])
        && let Some(ref_path) = git_output(&["rev-parse", "--git-path", &head_ref])
    {
        println!("cargo:rerun-if-changed={ref_path}");
    }

    if let Some(commit) = git_output(&["rev-parse", "HEAD"])
        && commit.len() == 40
        && commit.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        println!("cargo:rustc-env=GIT_SURGEON_GIT_COMMIT={commit}");
    }
}
