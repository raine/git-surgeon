# git-surgeon

`git-surgeon` gives AI agents surgical control over git changes without
interactive prompts. Stage, unstage, or discard individual hunks. Commit hunks
directly with line-range precision. Restructure history by splitting commits or
folding fixes into earlier ones.

## Why?

AI agents can't use interactive git commands like `git add -p`. When asked to
commit changes separately, they resort to destructive workarounds:

```
❯ commit the changes separately

⏺ Bash(git diff internal/bot/listing_handler.go)
  ⎿  diff --git i/internal/bot/listing_handler.go ...
     ... +83 lines

⏺ Let me reset and make the changes in two separate commits.

⏺ Bash(git checkout internal/bot/listing_handler.go)   ← about to destroy changes
  ⎿  Interrupted
```

With git-surgeon, the agent can inspect hunks and commit them selectively:

```
❯ commit the changes separately

⏺ I'll commit these as separate changes using git-surgeon.

⏺ Skill(git-surgeon)
  ⎿  Successfully loaded skill

⏺ Bash(git-surgeon hunks)
  ⎿  ac34353 internal/bot/listing_handler.go (+6 -3)
     15baf94 internal/bot/listing_handler.go (+10 -2)
     ...

⏺ Bash(git-surgeon commit ac34353 15baf94 7c6ef9e -m "allow edit commands during attribute input")

⏺ Bash(git-surgeon commit 4eefac8 bbba931 -m "add logging for attribute prompts")
```

**Highlights**

- Stage specific lines from a hunk, not just whole hunks
  ([example](#example-splitting-a-hunk-across-commits))
- Split commits that mix concerns into focused commits
  ([example](#example-splitting-a-commit-that-mixes-concerns))
- Retroactively split a combined commit into separate changes
  ([example](#example-splitting-a-commit-retroactively))

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

### 2. Install the AI assistant skill

```bash
# Claude Code
git-surgeon install-skill --claude

# OpenCode
git-surgeon install-skill --opencode

# Codex
git-surgeon install-skill --codex
```

Alternatively, for Claude Code via the plugin marketplace:

```bash
claude plugin marketplace add raine/git-surgeon
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
- [`reword`](#reword) — Change the commit message of an existing commit
- [`squash`](#squash) — Squash multiple commits into one
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

# Show full diff with line numbers (useful for small commits)
git-surgeon hunks --commit abc1234 --full
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

### `reword`

Changes the commit message of an existing commit without modifying its content.
Uses `git commit --amend` for HEAD, or an autosquash rebase for older commits.

```bash
# With subject + body
git-surgeon reword HEAD -m "subject" -m "body paragraph"

# Change an earlier commit's message
git-surgeon reword abc1234 -m "corrected message"
```

If the rebase hits a conflict, the repo is left in the conflict state for manual
resolution (`git rebase --continue` or `git rebase --abort`).

---

### `squash`

Combines commits from `<commit>` through HEAD into a single commit.

```bash
# Squash last 2 commits
git-surgeon squash HEAD~1 -m "combined feature"

# Squash last 3 commits with subject + body
git-surgeon squash HEAD~2 -m "Add user auth" -m "Implements JWT-based authentication."

# Squash from a specific commit
git-surgeon squash abc1234 -m "feature complete"

# Force squash even if range contains merge commits
git-surgeon squash HEAD~3 --force -m "squash with merges"
```

The target commit must be an ancestor of HEAD. If the range contains merge
commits, use `--force` to flatten them into the squashed commit. Uncommitted
changes are autostashed and restored after squashing. The author from the oldest
commit is preserved by default; use `--no-preserve-author` to use the current
user instead.

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
  --pick a1b2c3d e4f5678 -m "add pagination" \
  --rest-message "filter deleted users"

# With subject + body (multiple -m flags, like git commit)
git-surgeon split HEAD \
  --pick a1b2c3d -m "Add pagination" -m "Adds page and per_page params." \
  --rest-message "Other changes" --rest-message "Body paragraph."

# With line ranges (id:range inline syntax)
# Use commas for non-contiguous ranges
git-surgeon split abc1234 \
  --pick a1b2c3d:1-11,20-30 e4f5678 \
  -m "add pagination"

# Multiple splits (more than two commits)
git-surgeon split abc1234 \
  --pick a1b2c3d -m "add pagination" \
  --pick e4f5678 -m "filter deleted users" \
  --rest-message "remaining cleanup"
```

Each `--pick` group specifies hunk IDs (with optional `:start-end` line ranges)
followed by `-m`/`--message`. Use multiple `-m` flags to add a commit body
(joined with blank lines, like `git commit`). Remaining unpicked hunks are
committed with `--rest-message` (defaults to the original commit message if
omitted; supports multiple values for body).

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

## Example: splitting a commit that mixes concerns

A commit mixes three unrelated changes: logging, pagination, and active user
filtering. The agent uses `hunks --commit` to see the changes, `show --commit`
to examine line numbers, then `split` with line ranges to separate concerns:

```
❯ split the big pagination commit that mixes concerns into multiple commits

⏺ Skill(git-surgeon)
  ⎿  Successfully loaded skill

⏺ I'll help you split the pagination commit. Let me first look at the commit
  history to identify it, then examine its hunks.

⏺ Bash(git log --oneline -5)
  ⎿  dda5d0e Add readme
     945381f Add pagination, logging, and active user filtering
     386b2fe Initial app setup

⏺ Bash(git-surgeon hunks --commit 945381f --full)
  ⎿  a811273 config.py (+3 -1)
     1: DATABASE_URL = "sqlite:///app.db"
     2: SECRET_KEY = "dev-secret-key"
     … +76 lines (ctrl+o to expand)

⏺ I can see the commit mixes several concerns:
  1. Logging - LOG_LEVEL, LOG_FORMAT, logging imports, logger.info/warning calls
  2. Pagination - MAX_PAGE_SIZE, pagination logic in list_users
  3. Active user filtering - active field in User model and data, filtering in list_users

  Let me split this into three focused commits:

⏺ Bash(git-surgeon split 945381f \
        --pick a811273:4-6 8c1940f:2,5-6,34,37 \
        -m "Add logging infrastructure" \
        -m "Configure debug logging level and format, add request l…)
  ⎿  committed: Add logging infrastructure
     committed: Add active flag to users
     committed: Add pagination support

⏺ Done. Let me verify the split:

⏺ Bash(git log --oneline -5)
  ⎿  f5d7d3e Add readme
     ff73e29 Add pagination support
     d6d0a7b Add active flag to users
     … +2 lines (ctrl+o to expand)

⏺ The commit has been split into three focused commits:
  ┌─────────┬─────────────────────────────────────────────────────────────────────────────────────────────┐
  │ Commit  │                                         Description                                         │
  ├─────────┼─────────────────────────────────────────────────────────────────────────────────────────────┤
  │ 6f98f7e │ Add logging infrastructure - LOG_LEVEL, LOG_FORMAT config, logging imports and logger calls │
  ├─────────┼─────────────────────────────────────────────────────────────────────────────────────────────┤
  │ d6d0a7b │ Add active flag to users - active field in model/data, filtering logic                      │
  ├─────────┼─────────────────────────────────────────────────────────────────────────────────────────────┤
  │ ff73e29 │ Add pagination support - MAX_PAGE_SIZE config, page/per_page params, paginated response     │
  └─────────┴─────────────────────────────────────────────────────────────────────────────────────────────┘

✻ Sautéed for 35s
```

## Example: splitting a commit retroactively

An agent commits two unrelated changes together. The user asks to separate them
after the fact using `split`:

```
⏺ Bash(git add instance.rs mounts.rs && git commit -m "mount credentials to guest home and show mounts on VM creation")
  ⎿  [sandbox ddef0f1] mount credentials to guest home and show mounts on VM creation
      2 files changed, 30 insertions(+), 3 deletions(-)

⏺ Committed ddef0f1 on sandbox. Two changes:
  - Fixed credential mount to target guest home instead of host path
  - Added mount point output when creating a new VM

❯ commit them separately

⏺ Skill(git-surgeon)
  ⎿  Successfully loaded skill

⏺ Bash(git-surgeon hunks --commit HEAD)
  ⎿  2fc287e instance.rs fn ensure_vm_running (+15 -0)
       +            eprintln!("  Mounts:");
     … +13 more lines
     53c287a mounts.rs fn get_mounts (+3 -3)
       -    mount_path: host_home.join(".claude"),
       +    mount_path: PathBuf::from("/home/user.linux/.claude"),
     bdbda13 instance.rs fn create_vm (+12 -0)
       +    for mount in &mounts {
     … +10 more lines

⏺ Bash(git-surgeon split HEAD \
        --pick 53c287a bdbda13 -m "mount credentials to guest home directory" \
        --rest-message "show mount points when creating a new VM")
  ⎿  committed: mount credentials to guest home directory
     committed: show mount points when creating a new VM
```

## Requirements

- Git 2.0+
- Rust (for building from source)

## Related projects

- [workmux](https://github.com/raine/workmux) — Git worktrees + tmux windows for
  parallel AI agent workflows
- [claude-history](https://github.com/raine/claude-history) — Search and view
  Claude Code conversation history with fzf
