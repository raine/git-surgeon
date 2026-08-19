# Releasing

Requires [rust-release-tools](https://github.com/raine/rust-release-tools):

```bash
pipx install git+https://github.com/raine/rust-release-tools.git
```

To release:

```bash
just release-patch  # or release-minor, release-major
```

This will:

1. Bump version in Cargo.toml
2. Generate changelog entry using Claude
3. Open editor to review changelog
4. Commit, publish to crates.io, tag, and push

The binary owns the canonical companion skill in
`skills/git-surgeon/SKILL.md`. Before releasing, synchronize its
`cli_version` frontmatter and managed-file marker with the Cargo package
version, then run the full checks. Verify discovery and pi installation as part
of the release gate:

```bash
just check
cargo test --all
git-surgeon skill list --json
git-surgeon version --json
git-surgeon skill install --target pi --target-root "$(mktemp -d)" --dry-run
just test
```

Do not maintain runtime-specific downstream copies of the skill; the released
binary embeds and installs the canonical source for Claude Code, pi, OpenCode,
and Codex.

## Backfilling changelog

To generate changelog entries for all git tags missing from CHANGELOG.md:

```bash
update-changelog
```

This uses `cc-batch` to process multiple tags in parallel.
