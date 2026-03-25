# Rust project checks

set positional-arguments
set shell := ["bash", "-euo", "pipefail", "-c"]

# List available commands
default:
    @just --list

# Run format, clippy, and build in parallel
[parallel]
check: format clippy build

# Format Rust code
format:
    @cargo fmt --all

# Auto-fix clippy warnings, then fail on any remaining
clippy:
    @cargo clippy --fix --allow-dirty --quiet -- -D clippy::all 2>&1 | { grep -v "^0 errors" || true; }

# Build the project
build:
    cargo build --all

# Install release binary globally
install:
    cargo install --offline --path . --locked

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
    source tests/venv/bin/activate
    quiet_flag=""
    [[ -n "${CLAUDECODE:-}" ]] && quiet_flag="-q"
    if [ $# -eq 0 ]; then
        pytest tests/ -n auto $quiet_flag
    else
        pytest $quiet_flag "$@"
    fi

# Release a new patch version
release *ARGS:
    @just _release patch {{ARGS}}

# Internal release helper
_release bump *ARGS:
    @cargo-release {{bump}} {{ARGS}}
