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

## Backfilling changelog

To generate changelog entries for all git tags missing from CHANGELOG.md:

```bash
update-changelog
```

This uses `cc-batch` to process multiple tags in parallel.
