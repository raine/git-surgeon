# Rust project checks

set positional-arguments
set shell := ["bash", "-euo", "pipefail", "-c"]

# List available commands
default:
    @just --list

# Run format, clippy-fix, and build in parallel, then clippy
check: parallel-checks clippy

# Run format, clippy-fix, and build in parallel
[parallel]
parallel-checks: format clippy-fix build

# Format Rust code
format:
    cargo fmt --all

# Run clippy and fail on any warnings
clippy:
    cargo clippy -- -D clippy::all

# Auto-fix clippy warnings
clippy-fix:
    cargo clippy --fix --allow-dirty -- -W clippy::all

# Build the project
build:
    cargo build --all

# Install debug binary globally via symlink
install-dev:
    cargo build && ln -sf $(pwd)/target/debug/git-surgeon ~/.cargo/bin/git-surgeon

# Run the application
run *ARGS:
    cargo run -- "$@"

# Run Python integration tests (depends on build)
test *ARGS: build
    #!/usr/bin/env bash
    set -euo pipefail
    quiet_flag=""
    [[ -n "${CLAUDECODE:-}" ]] && quiet_flag="-q"
    if [ $# -eq 0 ]; then
        pytest tests/ $quiet_flag
    else
        pytest $quiet_flag "$@"
    fi
