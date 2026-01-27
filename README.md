# git-surgeon

Surgical, non-interactive git hunk control for AI agents.

Without hunk-level staging, an AI agent that needs to commit two independent
changes in the same file has to edit one change out, commit, then restore it.
git-surgeon lets the agent stage each hunk separately without gymnastics.

## Quick start

### 1. Install

```bash
# Shell
curl -fsSL https://raw.githubusercontent.com/raine/git-surgeon/main/scripts/install.sh | bash

# Cargo
cargo install git-surgeon

# Homebrew
brew install raine/git-surgeon/git-surgeon
```

### 2. Install the Claude Code plugin

```bash
# Register the plugin source
claude plugin marketplace add raine/git-surgeon
# Install the skill that teaches Claude Code to use git-surgeon
claude plugin install git-surgeon@git-surgeon
```

### 3. Use it

Ask Claude Code to make granular commits. It will use git-surgeon automatically
to stage individual hunks instead of entire files.

## Commands

- [`hunks`](#hunks) — List hunks in the diff
- [`show`](#show) — Show full diff for a specific hunk
- [`stage`](#stage) — Stage hunks by ID
- [`unstage`](#unstage) — Unstage hunks by ID
- [`discard`](#discard) — Discard working tree changes for hunks
- [`fixup`](#fixup) — Fold staged changes into an earlier commit
- [`undo`](#undo) — Reverse-apply hunks from a commit

---

### `hunks`

Lists all hunks with their IDs, file paths, function context, change counts, and
a preview of changed lines.

```bash
# List unstaged hunks
git-surgeon hunks

# List staged hunks
git-surgeon hunks --staged

# Filter to a specific file
git-surgeon hunks --file src/main.rs

# List hunks from a specific commit
git-surgeon hunks --commit HEAD
git-surgeon hunks --commit abc1234
```

#### Example output

```
a1b2c3d src/main.rs fn handle_request (+3 -1)
  -    let result = process(input);
  +    let result = match process(input) {
  +        Ok(v) => v,
  +        Err(e) => return Err(e),
  +    };

e4f5678 src/lib.rs (+1 -0)
  +use std::collections::HashMap;
```

Each line shows: `<hunk-id> <file> [function context] (+additions -deletions)`

---

### `show`

Shows the full diff (header + all lines) for a single hunk.

```bash
git-surgeon show a1b2c3d

# Show a hunk from a specific commit
git-surgeon show a1b2c3d --commit HEAD
```

Searches both unstaged and staged diffs when no `--commit` is specified.

---

### `stage`

Stages one or more hunks by ID. Equivalent to selectively answering "y" in
`git add -p`.

```bash
git-surgeon stage a1b2c3d
git-surgeon stage a1b2c3d e4f5678
```

---

### `unstage`

Unstages one or more previously staged hunks, moving them back to the working
tree.

```bash
git-surgeon unstage a1b2c3d
git-surgeon unstage a1b2c3d e4f5678
```

---

### `discard`

Discards working tree changes for specific hunks. This reverse-applies the
hunks, effectively running `git checkout -p` non-interactively.

```bash
git-surgeon discard a1b2c3d
```

**Warning:** This permanently removes uncommitted changes for the specified
hunks.

---

### `fixup`

Folds currently staged changes into an earlier commit. Uses `git commit --amend`
for HEAD, or an autosquash rebase for older commits. Unstaged changes are
preserved via `--autostash`.

```bash
# Stage some hunks, then fixup an earlier commit
git-surgeon stage a1b2c3d
git-surgeon fixup abc1234

# Fixup HEAD (equivalent to git commit --amend --no-edit)
git-surgeon fixup HEAD
```

If the rebase hits a conflict, the repo is left in the conflict state for manual
resolution (`git rebase --continue` or `git rebase --abort`).

---

### `undo`

Reverse-applies hunks from a specific commit onto the working tree. Useful for
selectively reverting parts of a previous commit without reverting the entire
commit.

```bash
# List hunks from the commit to find IDs
git-surgeon hunks --commit HEAD

# Undo specific hunks
git-surgeon undo a1b2c3d --from HEAD
git-surgeon undo a1b2c3d e4f5678 --from HEAD~3
```

The changes appear as unstaged modifications in the working tree. Fails
gracefully if context lines have changed since the commit (the patch no longer
applies cleanly).

## How hunk IDs work

IDs are 7-character hex strings derived from SHA-1 of the file path and hunk
content (the actual `+`/`-`/context lines, excluding the `@@` header). This
means:

- IDs are stable across line shifts — adding lines above a hunk doesn't change
  its ID
- IDs are deterministic — the same content always produces the same ID
- Collisions get a `-2`, `-3` suffix (e.g., `a1b2c3d-2`)

## Typical AI agent workflow

```bash
# 1. Agent makes changes to multiple files
# 2. Review what changed
git-surgeon hunks

# 3. Stage only the hunks related to feature A
git-surgeon stage a1b2c3d e4f5678

# 4. Commit feature A
git commit -m "implement feature A"

# 5. Stage remaining hunks for feature B
git-surgeon stage f6g7h8i
git commit -m "implement feature B"
```

## Requirements

- Git 2.0+
- Rust (for building from source)
