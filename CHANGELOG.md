# Changelog

## v0.1.7 (2026-02-07)

- Add `--blame` flag to `hunks` command to show which commit introduced each
  line

## v0.1.6 (2026-02-05)

- Improve documentation for `squash` and `fixup` commands

## v0.1.5 (2026-01-31)

- Improve skill trigger keywords so AI assistants recognize requests to "commit
  changes separately" or "make separate commits"

## v0.1.4 (2026-01-29)

- Add `reword` command to change commit messages non-interactively
- Add `squash` command to combine multiple commits into one without rebase
  conflicts
- Add `--full` flag to `hunks` to show all lines with line numbers in one call
- Support comma-separated line ranges in split and commit commands (e.g.,
  `id:2,5-6,34`)
- Preserve original author and date when squashing commits

## v0.1.3 (2026-01-29)

- Add `install-skill` command to install the AI assistant skill for Claude Code,
  OpenCode, and Codex

## v0.1.2 (2026-01-28)

- Add `commit` command to stage hunks and commit in one step
- Add `split` command to decompose commits into multiple commits by hunk
  selection
- Support inline line range syntax (`abc123:5-30`) in commit and split commands
- Support commit message body via multiple `-m` flags in split command
- Reject hunks with unsupported metadata (rename, copy, mode change) with clear
  error
- Fix split failing when a hunk ID has multiple line ranges
- Fix split failing when using multiple `--pick` groups
- Fix split picking wrong hunk when file has multiple hunks
- Fix file path resolution for deleted files in diff parsing

## v0.1.1 (2026-01-27)

- Display line numbers when showing hunks
- Add `--lines` flag for operating on specific lines within a hunk

## v0.1.0 (2026-01-27)

Initial release
