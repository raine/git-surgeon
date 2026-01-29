"""Tests for the squash command."""

import pytest
from conftest import run_git_agent, run_git


def write_file(repo, path, content):
    """Write a file without git add/commit."""
    filepath = repo / path
    filepath.parent.mkdir(parents=True, exist_ok=True)
    filepath.write_text(content)


@pytest.fixture
def exe(git_agent_exe):
    return git_agent_exe


def test_squash_two_commits(exe, repo):
    """Squash last two commits into one."""
    write_file(repo, "a.txt", "initial")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "first commit")

    write_file(repo, "b.txt", "second")
    run_git(repo, "add", "b.txt")
    run_git(repo, "commit", "-m", "second commit")

    # Squash both commits
    result = run_git_agent(exe, repo, "squash", "HEAD~1", "-m", "squashed commit")
    assert result.returncode == 0

    # Should have one commit now with both files (plus init commit)
    log = run_git(repo, "log", "--oneline")
    lines = [l for l in log.stdout.strip().split("\n") if l]
    assert len(lines) == 2  # init + squashed
    assert "squashed commit" in log.stdout

    # Both files should exist
    assert (repo / "a.txt").exists()
    assert (repo / "b.txt").exists()


def test_squash_three_commits(exe, repo):
    """Squash last three commits into one."""
    for i, name in enumerate(["a", "b", "c"]):
        write_file(repo, f"{name}.txt", f"content {i}")
        run_git(repo, "add", f"{name}.txt")
        run_git(repo, "commit", "-m", f"commit {name}")

    result = run_git_agent(exe, repo, "squash", "HEAD~2", "-m", "all three")
    assert result.returncode == 0

    log = run_git(repo, "log", "--oneline")
    lines = [l for l in log.stdout.strip().split("\n") if l]
    assert len(lines) == 2  # init + squashed
    assert "all three" in log.stdout


def test_squash_head_errors(exe, repo):
    """Squashing HEAD into itself should error."""
    write_file(repo, "a.txt", "content")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "only commit")

    result = run_git_agent(exe, repo, "squash", "HEAD", "-m", "nope")
    assert result.returncode != 0
    assert "nothing to squash" in result.stderr.lower() or "target commit is head" in result.stderr.lower()


def test_squash_autostashes_dirty_tree(exe, repo):
    """Squashing with modified tracked files should autostash and restore."""
    write_file(repo, "a.txt", "initial")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "first")

    write_file(repo, "b.txt", "second")
    run_git(repo, "add", "b.txt")
    run_git(repo, "commit", "-m", "second")

    # Make working tree dirty by modifying a tracked file
    write_file(repo, "a.txt", "modified")

    result = run_git_agent(exe, repo, "squash", "HEAD~1", "-m", "squashed")
    assert result.returncode == 0

    # Working tree modification should be restored
    assert (repo / "a.txt").read_text() == "modified"

    # Squash should have worked
    log = run_git(repo, "log", "--oneline")
    assert "squashed" in log.stdout


def test_squash_with_untracked_files_succeeds(exe, repo):
    """Squashing should succeed when untracked files are present."""
    write_file(repo, "a.txt", "initial")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "first")

    write_file(repo, "b.txt", "second")
    run_git(repo, "add", "b.txt")
    run_git(repo, "commit", "-m", "second")

    # Create untracked file (should not block squash)
    write_file(repo, "untracked.txt", "untracked content")

    result = run_git_agent(exe, repo, "squash", "HEAD~1", "-m", "squashed")
    assert result.returncode == 0

    # Untracked file should still exist
    assert (repo / "untracked.txt").exists()


def test_squash_merge_commits_errors(exe, repo):
    """Squashing range with merge commits should error without --force."""
    write_file(repo, "a.txt", "main")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "initial on main")

    # Create feature branch
    run_git(repo, "checkout", "-b", "feature")
    write_file(repo, "b.txt", "feature")
    run_git(repo, "add", "b.txt")
    run_git(repo, "commit", "-m", "feature commit")

    # Back to main, add another commit
    run_git(repo, "checkout", "main")
    write_file(repo, "c.txt", "main2")
    run_git(repo, "add", "c.txt")
    run_git(repo, "commit", "-m", "second on main")

    # Merge feature into main
    run_git(repo, "merge", "feature", "-m", "merge feature")

    # Try to squash (includes merge commit)
    result = run_git_agent(exe, repo, "squash", "HEAD~2", "-m", "squashed")
    assert result.returncode != 0
    assert "merge" in result.stderr.lower()


def test_squash_merge_commits_with_force(exe, repo):
    """Squashing range with merge commits should succeed with --force."""
    write_file(repo, "a.txt", "main")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "initial on main")

    # Create feature branch
    run_git(repo, "checkout", "-b", "feature")
    write_file(repo, "b.txt", "feature")
    run_git(repo, "add", "b.txt")
    run_git(repo, "commit", "-m", "feature commit")

    # Back to main, add another commit
    run_git(repo, "checkout", "main")
    write_file(repo, "c.txt", "main2")
    run_git(repo, "add", "c.txt")
    run_git(repo, "commit", "-m", "second on main")

    # Merge feature into main
    run_git(repo, "merge", "feature", "-m", "merge feature")

    # Squash with --force
    result = run_git_agent(exe, repo, "squash", "HEAD~2", "--force", "-m", "squashed")
    assert result.returncode == 0

    # All files should exist
    assert (repo / "a.txt").exists()
    assert (repo / "b.txt").exists()
    assert (repo / "c.txt").exists()


def test_squash_not_ancestor_errors(exe, repo):
    """Squashing non-ancestor commit should error."""
    write_file(repo, "a.txt", "main")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "main commit")

    # Create a branch with different history
    run_git(repo, "checkout", "-b", "other")
    write_file(repo, "b.txt", "other")
    run_git(repo, "add", "b.txt")
    run_git(repo, "commit", "-m", "other commit")
    other_sha = run_git(repo, "rev-parse", "HEAD").stdout.strip()

    # Go back to main
    run_git(repo, "checkout", "main")
    write_file(repo, "c.txt", "more main")
    run_git(repo, "add", "c.txt")
    run_git(repo, "commit", "-m", "more main")

    # Try to squash non-ancestor
    result = run_git_agent(exe, repo, "squash", other_sha[:7], "-m", "nope")
    assert result.returncode != 0
    assert "not an ancestor" in result.stderr.lower()


def test_squash_preserves_author_and_date_by_default(exe, repo):
    """Squash should preserve the author and date from the oldest commit by default."""
    # Set up a commit with a different author
    run_git(repo, "config", "user.name", "Original Author")
    run_git(repo, "config", "user.email", "original@example.com")

    write_file(repo, "a.txt", "initial")
    run_git(repo, "add", "a.txt")
    # Use a specific date in the past
    run_git(repo, "commit", "-m", "first commit", "--date", "2020-01-15T10:30:00")

    # Get the original date
    original_date = run_git(repo, "log", "-1", "--format=%aI").stdout.strip()

    write_file(repo, "b.txt", "second")
    run_git(repo, "add", "b.txt")
    run_git(repo, "commit", "-m", "second commit")

    # Change to a different user for the squash
    run_git(repo, "config", "user.name", "Current User")
    run_git(repo, "config", "user.email", "current@example.com")

    result = run_git_agent(exe, repo, "squash", "HEAD~1", "-m", "squashed")
    assert result.returncode == 0

    # Check that the author is preserved from the first commit
    author = run_git(repo, "log", "-1", "--format=%an <%ae>").stdout.strip()
    assert author == "Original Author <original@example.com>"

    # Check that the date is preserved (should match the original, not current time)
    squashed_date = run_git(repo, "log", "-1", "--format=%aI").stdout.strip()
    assert squashed_date == original_date


def test_squash_no_preserve_author(exe, repo):
    """Squash with --no-preserve-author uses current user."""
    # Set up a commit with a different author
    run_git(repo, "config", "user.name", "Original Author")
    run_git(repo, "config", "user.email", "original@example.com")

    write_file(repo, "a.txt", "initial")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "first commit")

    write_file(repo, "b.txt", "second")
    run_git(repo, "add", "b.txt")
    run_git(repo, "commit", "-m", "second commit")

    # Change to a different user for the squash
    run_git(repo, "config", "user.name", "Current User")
    run_git(repo, "config", "user.email", "current@example.com")

    result = run_git_agent(exe, repo, "squash", "HEAD~1", "--no-preserve-author", "-m", "squashed")
    assert result.returncode == 0

    # Check that the author is the current user
    author = run_git(repo, "log", "-1", "--format=%an <%ae>").stdout.strip()
    assert author == "Current User <current@example.com>"
