---
name: git-surgeon
description: Non-interactive hunk-level git staging, unstaging, discarding, undoing, and fixup. Use when selectively staging, unstaging, discarding, or reverting individual diff hunks by ID instead of interactively.
---

# git-surgeon

CLI for hunk-level git operations without interactive prompts. Useful for AI
agents that need precise control over which changes to stage, unstage, discard,
or undo.

## Commands

```bash
# List unstaged hunks (shows ID, file, +/- counts, preview)
git-surgeon hunks

# List staged hunks
git-surgeon hunks --staged

# Filter to one file
git-surgeon hunks --file=src/main.rs

# List hunks from a specific commit
git-surgeon hunks --commit <HEAD/sha>

# Show full diff for a hunk (lines are numbered for use with --lines)
git-surgeon show <id>
git-surgeon show <id> --commit HEAD

# Stage specific hunks
git-surgeon stage <id1> <id2> ...

# Stage only part of a hunk by line range
git-surgeon stage <id> --lines 5-30

# Stage and commit hunks in one step
git-surgeon commit <id1> <id2> ... -m "message"

# With inline line ranges
git-surgeon commit <id>:1-11 <id2> -m "message"

# Unstage specific hunks
git-surgeon unstage <id1> <id2> ...
git-surgeon unstage <id> --lines 5-30

# Discard working tree changes for specific hunks
git-surgeon discard <id1> <id2> ...
git-surgeon discard <id> --lines 5-30

# Fixup an earlier commit with currently staged changes
git-surgeon fixup <commit>

# Undo specific hunks from a commit (reverse-apply to working tree)
git-surgeon undo <id1> <id2> ... --from <commit>
git-surgeon undo <id> --from <commit> --lines 2-10

# Undo all changes to specific files from a commit
git-surgeon undo-file <file1> <file2> ... --from <commit>
```

## Typical workflow

1. Run `git-surgeon hunks` to list hunks with their IDs
2. Use `git-surgeon show <id>` to inspect a hunk (lines are numbered)
3. Stage and commit in one step: `git-surgeon commit <id1> <id2> -m "message"`
4. Or stage separately: `git-surgeon stage <id1> <id2>`, then `git commit`
5. To commit only part of a hunk, use inline ranges: `git-surgeon commit <id>:5-30 -m "message"`

## Fixing up earlier commits

1. Stage desired hunks: `git-surgeon stage <id1> <id2>`
2. Fixup the target commit: `git-surgeon fixup <commit-sha>`
3. For HEAD, this amends directly; for older commits, it uses autosquash rebase
4. Unstaged changes are preserved automatically

## Undoing changes from commits

1. Run `git-surgeon hunks --commit <sha>` to list hunks in a commit
2. Undo specific hunks: `git-surgeon undo <id> --from <sha>`
3. Or undo entire files: `git-surgeon undo-file src/main.rs --from <sha>`
4. Changes appear as unstaged modifications in the working tree

## Hunk IDs

- 7-character hex strings derived from file path + hunk content
- Stable across runs as long as the diff content hasn't changed
- Duplicates get `-2`, `-3` suffixes
- If a hunk ID is not found, re-run `hunks` to get fresh IDs
