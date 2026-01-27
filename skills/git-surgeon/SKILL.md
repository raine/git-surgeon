---
name: git-surgeon
description: Non-interactive hunk-level git staging, unstaging, and discarding. Use when selectively staging, unstaging, or discarding individual diff hunks by ID instead of interactively.
---

# git-surgeon

CLI for hunk-level git operations without interactive prompts. Useful for AI agents that need precise control over which changes to stage, unstage, or discard.

## Commands

```bash
# List unstaged hunks (shows ID, file, +/- counts, preview)
git-surgeon hunks

# List staged hunks
git-surgeon hunks --staged

# Filter to one file
git-surgeon hunks --file=src/main.rs

# Show full diff for a hunk
git-surgeon show <id>

# Stage specific hunks
git-surgeon stage <id1> <id2> ...

# Unstage specific hunks
git-surgeon unstage <id1> <id2> ...

# Discard working tree changes for specific hunks
git-surgeon discard <id1> <id2> ...
```

## Typical workflow

1. Run `git-surgeon hunks` to list hunks with their IDs
2. Use `git-surgeon show <id>` to inspect a hunk if needed
3. Stage desired hunks: `git-surgeon stage <id1> <id2>`
4. Commit staged changes with `git commit`

## Hunk IDs

- 7-character hex strings derived from file path + hunk content
- Stable across runs as long as the diff content hasn't changed
- Duplicates get `-2`, `-3` suffixes
- If a hunk ID is not found, re-run `hunks` to get fresh IDs
