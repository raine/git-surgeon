"""Tests for the move command (reorder commits in history)."""

from conftest import run_git_agent, run_git, create_file, modify_file


def _commit_sha(repo, ref="HEAD"):
    result = run_git(repo, "rev-parse", ref)
    return result.stdout.strip()


def _commit_subjects(repo):
    """Return list of commit subjects oldest-first."""
    result = run_git(repo, "log", "--reverse", "--format=%s")
    return [s for s in result.stdout.strip().split("\n") if s]


def test_move_forward(git_agent_exe, repo):
    """Move an earlier commit to after a later commit."""
    create_file(repo, "a.txt", "aaa\n")
    a_sha = _commit_sha(repo)
    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")
    c_sha = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "move", a_sha, "--after", c_sha)
    assert result.returncode == 0

    subjects = _commit_subjects(repo)
    a_idx = subjects.index("add a.txt")
    c_idx = subjects.index("add c.txt")
    assert a_idx == c_idx + 1, f"expected a.txt after c.txt, got order: {subjects}"


def test_move_backward(git_agent_exe, repo):
    """Move a later commit to after an earlier commit."""
    create_file(repo, "a.txt", "aaa\n")
    a_sha = _commit_sha(repo)
    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")
    c_sha = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "move", c_sha, "--after", a_sha)
    assert result.returncode == 0

    subjects = _commit_subjects(repo)
    a_idx = subjects.index("add a.txt")
    c_idx = subjects.index("add c.txt")
    assert c_idx == a_idx + 1, f"expected c.txt after a.txt, got order: {subjects}"


def test_move_before(git_agent_exe, repo):
    """Move a commit before another commit."""
    create_file(repo, "a.txt", "aaa\n")
    a_sha = _commit_sha(repo)
    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")
    c_sha = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "move", c_sha, "--before", a_sha)
    assert result.returncode == 0

    subjects = _commit_subjects(repo)
    a_idx = subjects.index("add a.txt")
    c_idx = subjects.index("add c.txt")
    assert c_idx == a_idx - 1, f"expected c.txt before a.txt, got order: {subjects}"


def test_move_to_end(git_agent_exe, repo):
    """Move a commit to the end of the branch."""
    create_file(repo, "a.txt", "aaa\n")
    a_sha = _commit_sha(repo)
    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")

    result = run_git_agent(git_agent_exe, repo, "move", a_sha, "--to-end")
    assert result.returncode == 0

    subjects = _commit_subjects(repo)
    assert subjects[-1] == "add a.txt"


def test_move_preserves_content(git_agent_exe, repo):
    """Moving a commit preserves its content and message."""
    create_file(repo, "a.txt", "aaa\n")
    a_sha = _commit_sha(repo)
    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")
    c_sha = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "move", a_sha, "--after", c_sha)
    assert result.returncode == 0

    # All files should still exist with correct content
    assert (repo / "a.txt").read_text() == "aaa\n"
    assert (repo / "b.txt").read_text() == "bbb\n"
    assert (repo / "c.txt").read_text() == "ccc\n"

    # All commit subjects should be preserved
    subjects = _commit_subjects(repo)
    assert "add a.txt" in subjects
    assert "add b.txt" in subjects
    assert "add c.txt" in subjects


def test_move_same_commit_errors(git_agent_exe, repo):
    """Move with source == target should error."""
    create_file(repo, "a.txt", "aaa\n")
    a_sha = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "move", a_sha, "--after", a_sha)
    assert result.returncode != 0
    assert "same commit" in result.stderr.lower()


def test_move_preserves_dirty_worktree(git_agent_exe, repo):
    """Move should autostash and restore dirty working tree."""
    create_file(repo, "a.txt", "aaa\n")
    a_sha = _commit_sha(repo)
    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")
    c_sha = _commit_sha(repo)

    # Make working tree dirty
    modify_file(repo, "b.txt", "bbb modified\n")

    result = run_git_agent(git_agent_exe, repo, "move", a_sha, "--after", c_sha)
    assert result.returncode == 0

    # Dirty file should be restored
    assert (repo / "b.txt").read_text() == "bbb modified\n"


def test_move_already_at_end(git_agent_exe, repo):
    """Moving HEAD with --to-end should be a no-op."""
    create_file(repo, "a.txt", "aaa\n")

    result = run_git_agent(git_agent_exe, repo, "move", "HEAD", "--to-end")
    assert result.returncode == 0
    assert "already at head" in result.stderr.lower()


def test_move_merge_commit_errors(git_agent_exe, repo):
    """Move with merge commits in range should error."""
    create_file(repo, "a.txt", "main\n")
    a_sha = _commit_sha(repo)

    run_git(repo, "checkout", "-b", "feature")
    create_file(repo, "b.txt", "feature\n")

    run_git(repo, "checkout", "main")
    create_file(repo, "c.txt", "main2\n")
    run_git(repo, "merge", "feature", "-m", "merge feature")

    create_file(repo, "d.txt", "ddd\n")
    d_sha = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "move", a_sha, "--after", d_sha)
    assert result.returncode != 0
    assert "merge" in result.stderr.lower()


def test_move_no_position_errors(git_agent_exe, repo):
    """Move without position flag should error."""
    create_file(repo, "a.txt", "aaa\n")
    a_sha = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "move", a_sha)
    assert result.returncode != 0
