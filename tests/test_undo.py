from conftest import run_git_agent, run_git, modify_file


def test_hunks_with_commit_flag(repo, git_agent_exe):
    """List hunks from a specific commit."""
    (repo / "file.txt").write_text("line1\nline2\nline3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add file")

    modify_file(repo, "file.txt", "line1\nchanged\nline3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify line2")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--commit", "HEAD")
    assert result.returncode == 0
    assert "file.txt" in result.stdout
    assert "+changed" in result.stdout


def test_undo_hunk_clean_tree(repo, git_agent_exe):
    """Undo a hunk from an earlier commit onto clean working tree."""
    (repo / "file.txt").write_text("line1\nline2\nline3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add file")

    modify_file(repo, "file.txt", "line1\nchanged\nline3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify line2")

    # Get hunk ID
    result = run_git_agent(git_agent_exe, repo, "hunks", "--commit", "HEAD")
    hunk_id = result.stdout.strip().split()[0]

    # Undo it
    result = run_git_agent(git_agent_exe, repo, "undo", hunk_id, "--from", "HEAD")
    assert result.returncode == 0

    # Working tree should now have the reverse change
    diff = run_git(repo, "diff")
    assert "-changed" in diff.stdout
    assert "+line2" in diff.stdout


def test_undo_hunk_with_existing_changes(repo, git_agent_exe):
    """Undo a hunk when there are already unstaged changes."""
    (repo / "file.txt").write_text("line1\nline2\nline3\n")
    (repo / "other.txt").write_text("hello\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add files")

    modify_file(repo, "file.txt", "line1\nchanged\nline3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify line2")

    # Make an unrelated unstaged change
    modify_file(repo, "other.txt", "world\n")

    # Get hunk ID and undo
    result = run_git_agent(git_agent_exe, repo, "hunks", "--commit", "HEAD")
    hunk_id = result.stdout.strip().split()[0]
    result = run_git_agent(git_agent_exe, repo, "undo", hunk_id, "--from", "HEAD")
    assert result.returncode == 0

    # Both changes should be in working tree
    diff = run_git(repo, "diff")
    assert "+line2" in diff.stdout
    assert "+world" in diff.stdout


def test_undo_hunk_context_mismatch(repo, git_agent_exe):
    """Undo fails gracefully when context lines have changed."""
    (repo / "file.txt").write_text("line1\nline2\nline3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add file")

    modify_file(repo, "file.txt", "line1\nchanged\nline3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify line2")

    # Later commit modifies context lines
    modify_file(repo, "file.txt", "LINE1\nchanged\nLINE3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify surrounding lines")

    # Get hunk ID from the middle commit
    result = run_git_agent(git_agent_exe, repo, "hunks", "--commit", "HEAD~1")
    hunk_id = result.stdout.strip().split()[0]

    # Undo should fail because context no longer matches
    result = run_git_agent(git_agent_exe, repo, "undo", hunk_id, "--from", "HEAD~1")
    assert result.returncode != 0
