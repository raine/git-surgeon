from conftest import run_git_agent, run_git, create_file, modify_file


def test_blame_basic_unstaged(git_agent_exe, repo):
    """Basic blamed diff: context/removed lines show hashes, added shows 0000000."""
    create_file(repo, "test.txt", "line1\nline2\nline3\n")
    modify_file(repo, "test.txt", "line1\nmodified\nline3\n")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame")
    assert result.returncode == 0

    lines = result.stdout.strip().split("\n")
    # Find blamed lines (indented, with hash prefix)
    blamed_lines = [l for l in lines if l.startswith("  ") and len(l) > 10]

    # Should have context and changed lines with hashes
    assert len(blamed_lines) >= 3

    # Added lines should show 0000000
    added_lines = [l for l in blamed_lines if "+modified" in l]
    assert len(added_lines) == 1
    assert "0000000" in added_lines[0]

    # Removed lines should show a real commit hash (not 0000000)
    removed_lines = [l for l in blamed_lines if "-line2" in l]
    assert len(removed_lines) == 1
    assert "0000000" not in removed_lines[0]


def test_blame_staged(git_agent_exe, repo):
    """Staged changes with blame work correctly."""
    create_file(repo, "staged.txt", "original\n")
    modify_file(repo, "staged.txt", "modified\n")
    run_git(repo, "add", "staged.txt")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame", "--staged")
    assert result.returncode == 0
    assert "staged.txt" in result.stdout

    # Added line should be 0000000
    lines = result.stdout.strip().split("\n")
    added = [l for l in lines if "+modified" in l]
    assert len(added) == 1
    assert "0000000" in added[0]


def test_blame_commit(git_agent_exe, repo):
    """Blame for a specific commit shows commit hashes for additions."""
    create_file(repo, "commit.txt", "line1\n")
    modify_file(repo, "commit.txt", "line1\nline2\n")
    run_git(repo, "add", "commit.txt")
    run_git(repo, "commit", "-m", "add line2")

    # Get the commit hash
    result = run_git(repo, "rev-parse", "--short=7", "HEAD")
    commit_hash = result.stdout.strip()

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame", "--commit=HEAD")
    assert result.returncode == 0

    # Added line should show the commit hash, not 0000000
    lines = result.stdout.strip().split("\n")
    added = [l for l in lines if "+line2" in l]
    assert len(added) == 1
    assert commit_hash in added[0]


def test_blame_new_file_staged(git_agent_exe, repo):
    """New file (staged): all lines show 0000000."""
    (repo / "newfile.txt").write_text("new content\n")
    run_git(repo, "add", "newfile.txt")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame", "--staged")
    assert result.returncode == 0

    lines = result.stdout.strip().split("\n")
    added = [l for l in lines if "+new content" in l]
    assert len(added) == 1
    assert "0000000" in added[0]


def test_blame_deleted_file(git_agent_exe, repo):
    """Deleted file: removed lines show the introducing commit hash."""
    create_file(repo, "todelete.txt", "content\n")

    # Get the commit hash that added the file
    result = run_git(repo, "rev-parse", "--short=7", "HEAD")
    add_hash = result.stdout.strip()

    (repo / "todelete.txt").unlink()

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame")
    assert result.returncode == 0

    lines = result.stdout.strip().split("\n")
    removed = [l for l in lines if "-content" in l]
    assert len(removed) == 1
    assert add_hash in removed[0]


def test_blame_with_file_filter(git_agent_exe, repo):
    """Blame works with --file filter."""
    create_file(repo, "a.txt", "a\n")
    create_file(repo, "b.txt", "b\n")
    modify_file(repo, "a.txt", "a modified\n")
    modify_file(repo, "b.txt", "b modified\n")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame", "--file=a.txt")
    assert result.returncode == 0
    assert "a.txt" in result.stdout
    assert "b.txt" not in result.stdout


def test_blame_multi_hunk(git_agent_exe, repo):
    """Multiple hunks in same file each get correct blame."""
    content = "line1\n" + "mid\n" * 20 + "line_end\n"
    create_file(repo, "multi.txt", content)
    new_content = "line1_changed\n" + "mid\n" * 20 + "line_end_changed\n"
    modify_file(repo, "multi.txt", new_content)

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame")
    assert result.returncode == 0

    # Should have multiple hunks
    assert result.stdout.count("multi.txt") >= 2

    # Each hunk should have blamed lines (indented with hash)
    lines = result.stdout.strip().split("\n")
    blamed_lines = [l for l in lines if l.startswith("  ") and len(l) > 10]
    assert len(blamed_lines) >= 4  # At least 2 changes per hunk


def test_blame_root_commit(git_agent_exe, repo):
    """Blame for root commit works (no parent to compare against)."""
    # The repo fixture creates an initial commit with .gitkeep
    # Create a new file and commit it
    create_file(repo, "root_test.txt", "root content\n")

    # Find root commit
    result = run_git(repo, "rev-list", "--max-parents=0", "HEAD")
    root_sha = result.stdout.strip().split("\n")[0]

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame", f"--commit={root_sha}")
    # Should not crash, may have empty output or warning
    assert result.returncode == 0


def test_blame_content_looks_like_hash(git_agent_exe, repo):
    """File content that looks like a git hash should not confuse blame parsing."""
    # Create a file with content that looks like a 40-char hex hash
    hash_like_content = "0123456789abcdef0123456789abcdef01234567 this looks like a hash\n"
    create_file(repo, "hashes.txt", hash_like_content)
    modify_file(repo, "hashes.txt", hash_like_content + "new line\n")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--blame")
    assert result.returncode == 0

    # The context line should have the real commit hash, not the fake one
    lines = result.stdout.strip().split("\n")
    context_lines = [l for l in lines if "this looks like a hash" in l]
    assert len(context_lines) == 1
    # The prefix should be a real 7-char hash, not 0123456
    assert not context_lines[0].strip().startswith("0123456")
