# Changelog

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
