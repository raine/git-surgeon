# git-surgeon

Surgical, non-interactive git hunk control for AI agents.

AI coding agents write code across multiple files but commit everything at once.
`git-surgeon` gives them the same selective staging power as `git add -p`, but
without the interactive prompts. Agents can list hunks, stage specific changes
by ID, and even split a single hunk across commits using line ranges.

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

### 2. Install the Claude Code Skill

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
- [`commit`](#commit) — Stage hunks and commit in one step
- [`unstage`](#unstage) — Unstage hunks by ID
- [`discard`](#discard) — Discard working tree changes for hunks
- [`fixup`](#fixup) — Fold staged changes into an earlier commit
- [`undo`](#undo) — Reverse-apply hunks from a commit
- [`split`](#split) — Split a commit into multiple commits by hunk selection

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

Shows the full diff (header + all lines) for a single hunk. Each line is
prefixed with a 1-based line number for use with `--lines`.

```bash
git-surgeon show a1b2c3d

# Show a hunk from a specific commit
git-surgeon show a1b2c3d --commit HEAD
```

#### Example output

```
@@ -1,4 +1,6 @@ fn main
1: context
2:-deleted line
3:+added line
4: context
```

Searches both unstaged and staged diffs when no `--commit` is specified.

---

### `stage`

Stages one or more hunks by ID. Equivalent to selectively answering "y" in
`git add -p`.

```bash
git-surgeon stage a1b2c3d
git-surgeon stage a1b2c3d e4f5678

# Stage only lines 5-30 of a hunk
git-surgeon stage a1b2c3d --lines 5-30
```

---

### `commit`

Stages hunks and commits them in a single step. Equivalent to running `stage`
followed by `git commit`. If the commit fails, the hunks are unstaged to restore
the original state. Refuses to run if the index already contains staged changes.

```bash
git-surgeon commit a1b2c3d e4f5678 -m "add pagination"

# With inline line ranges
git-surgeon commit a1b2c3d:1-11 e4f5678 -m "add pagination"
```

---

### `unstage`

Unstages one or more previously staged hunks, moving them back to the working
tree.

```bash
git-surgeon unstage a1b2c3d
git-surgeon unstage a1b2c3d e4f5678

# Unstage only lines 5-30 of a hunk
git-surgeon unstage a1b2c3d --lines 5-30
```

---

### `discard`

Discards working tree changes for specific hunks. This reverse-applies the
hunks, effectively running `git checkout -p` non-interactively.

```bash
git-surgeon discard a1b2c3d

# Discard only lines 5-30 of a hunk
git-surgeon discard a1b2c3d --lines 5-30
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

# Undo only lines 2-10 of a hunk
git-surgeon undo a1b2c3d --from HEAD --lines 2-10
```

The changes appear as unstaged modifications in the working tree. Fails
gracefully if context lines have changed since the commit (the patch no longer
applies cleanly).

---

### `split`

Splits an existing commit into multiple commits by selecting which hunks go into
each new commit. Works on HEAD (direct reset) or earlier commits (via rebase).

```bash
# Split HEAD into two commits
git-surgeon split HEAD \
  --pick a1b2c3d e4f5678 --message "add pagination" \
  --rest-message "filter deleted users"

# With line ranges (id:range inline syntax)
# Repeat same ID for non-contiguous ranges
git-surgeon split abc1234 \
  --pick a1b2c3d:1-11 a1b2c3d:20-30 e4f5678 \
  --message "add pagination"

# Multiple splits (more than two commits)
git-surgeon split abc1234 \
  --pick a1b2c3d --message "add pagination" \
  --pick e4f5678 --message "filter deleted users" \
  --rest-message "remaining cleanup"
```

Each `--pick` group specifies hunk IDs (with optional `:start-end` line ranges)
followed by a `--message`. Remaining unpicked hunks are committed with
`--rest-message` (defaults to the original commit message if omitted).

Requires a clean working tree. For non-HEAD commits, uses interactive rebase
with `--autostash`.

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

# 3. Stage and commit hunks for feature A in one step
git-surgeon commit a1b2c3d e4f5678 -m "implement feature A"

# 4. Commit remaining hunks for feature B
git-surgeon commit f6g7h8i -m "implement feature B"
```

## Example: splitting a hunk across commits

A file has two unrelated changes (pagination + soft-delete filtering) that git
groups into a single hunk. The agent uses `show` to see line numbers, then
`--lines` to stage only the pagination part:

```
❯ Commit the changes separately

⏺ Bash(git diff)
  ⎿  diff --git i/users.py w/users.py
     index c3c39df..24929a5 100644
     --- i/users.py
     +++ w/users.py
     @@ -1,13 +1,17 @@
      import json

     -def list_users(db):
     +def list_users(db, page=1, per_page=50):
          """Return all active users."""
     -    users = db.query("SELECT * FROM users WHERE active = 1")
     -    return users
     +    offset = (page - 1) * per_page
     +    users = db.query("SELECT * FROM users WHERE active = 1 LIMIT ? OFFSET ?", per_page, offset)
     +    total = db.query("SELECT COUNT(*) FROM users WHERE active = 1")[0]
     +    return {"users": users, "total": total, "page": page}

      def get_user(db, user_id):
          """Fetch a single user by ID."""
     -    user = db.query("SELECT * FROM users WHERE id = ?", user_id)
     +    user = db.query("SELECT * FROM users WHERE id = ? AND deleted_at IS NULL", user_id)
     +    if not user:
     +        return None
          return user

⏺ There are two logical changes here: pagination for list_users and
  soft-delete filtering + null handling for get_user. Let me commit them
  separately using git-surgeon.

⏺ Skill(git-surgeon)
  ⎿  Successfully loaded skill

⏺ Bash(git-surgeon hunks)
  ⎿  64ba255 users.py (+8 -4)
       -def list_users(db):
       +def list_users(db, page=1, per_page=50):
       -    users = db.query("SELECT * FROM users WHERE active = 1")
       ... (+8 more lines)

⏺ Only one hunk. Let me see the full diff with line numbers to split it.

⏺ Bash(git-surgeon show 64ba255)
  ⎿  @@ -1,13 +1,17 @@
      1: import json
      2:
      3:-def list_users(db):
      4:+def list_users(db, page=1, per_page=50):
     ... +15 lines (ctrl+o to expand)

⏺ Bash(git-surgeon stage 64ba255 --lines 1-11)
  ⎿  64ba255

⏺ Bash(git commit -m "add pagination to list_users")
  ⎿  [feature-users e81ede5] add pagination to list_users
      1 file changed, 5 insertions(+), 3 deletions(-)

⏺ Bash(git add users.py && git commit -m "filter soft-deleted users in get_user")
  ⎿  [feature-users de99d59] filter soft-deleted users in get_user
      1 file changed, 3 insertions(+), 1 deletion(-)
```

## Requirements

- Git 2.0+
- Rust (for building from source)
