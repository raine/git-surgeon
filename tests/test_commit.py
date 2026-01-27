from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def test_commit_single_hunk(git_agent_exe, repo):
    create_file(repo, "c.txt", "original\n")
    modify_file(repo, "c.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 1

    result = run_git_agent(git_agent_exe, repo, "commit", ids[0], "-m", "test commit")
    assert result.returncode == 0

    # Change should be committed (no staged, no unstaged)
    staged = run_git(repo, "diff", "--cached")
    assert staged.stdout.strip() == ""
    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() == ""

    # Verify commit message
    log = run_git(repo, "log", "-1", "--format=%s")
    assert log.stdout.strip() == "test commit"


def test_commit_multiple_hunks(git_agent_exe, repo):
    create_file(repo, "a.txt", "a\n")
    create_file(repo, "b.txt", "b\n")
    modify_file(repo, "a.txt", "a changed\n")
    modify_file(repo, "b.txt", "b changed\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 2

    result = run_git_agent(git_agent_exe, repo, "commit", *ids, "-m", "two hunks")
    assert result.returncode == 0

    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() == ""

    log = run_git(repo, "log", "-1", "--format=%s")
    assert log.stdout.strip() == "two hunks"


def test_commit_with_inline_range(git_agent_exe, repo):
    content = "line1\nline2\nline3\nline4\n"
    create_file(repo, "r.txt", content)
    modify_file(repo, "r.txt", "LINE1\nLINE2\nline3\nline4\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 1

    # Use id:range syntax to commit only part of the hunk
    result = run_git_agent(git_agent_exe, repo, "commit", f"{ids[0]}:1-2", "-m", "partial")
    assert result.returncode == 0

    log = run_git(repo, "log", "-1", "--format=%s")
    assert log.stdout.strip() == "partial"


def test_commit_multiple_m_flags(git_agent_exe, repo):
    """Multiple -m flags are joined by blank lines, like git commit."""
    create_file(repo, "mm.txt", "original\n")
    modify_file(repo, "mm.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    result = run_git_agent(git_agent_exe, repo, "commit", ids[0], "-m", "subject", "-m", "body text")
    assert result.returncode == 0

    log = run_git(repo, "log", "-1", "--format=%B")
    body = log.stdout.strip()
    assert body.startswith("subject")
    assert "body text" in body


def test_commit_invalid_id(git_agent_exe, repo):
    result = run_git_agent(git_agent_exe, repo, "commit", "invalid", "-m", "nope")
    assert result.returncode != 0


def test_commit_rejects_dirty_index(git_agent_exe, repo):
    """Commit should refuse if the index already has staged changes."""
    create_file(repo, "x.txt", "x\n")
    create_file(repo, "y.txt", "y\n")
    modify_file(repo, "x.txt", "x changed\n")
    modify_file(repo, "y.txt", "y changed\n")

    # Stage x.txt manually
    run_git(repo, "add", "x.txt")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) >= 1

    result = run_git_agent(git_agent_exe, repo, "commit", ids[0], "-m", "should fail")
    assert result.returncode != 0
    assert "staged changes" in result.stderr


def test_commit_leaves_other_hunks_unstaged(git_agent_exe, repo):
    """Committing one hunk should leave other hunks as unstaged changes."""
    content = "line1\n" + "mid\n" * 20 + "line_end\n"
    create_file(repo, "m.txt", content)
    new_content = "line1_changed\n" + "mid\n" * 20 + "line_end_changed\n"
    modify_file(repo, "m.txt", new_content)

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) >= 2

    # Commit only the first hunk
    result = run_git_agent(git_agent_exe, repo, "commit", ids[0], "-m", "first hunk only")
    assert result.returncode == 0

    # Should still have unstaged changes
    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() != ""

    # No staged changes
    staged = run_git(repo, "diff", "--cached")
    assert staged.stdout.strip() == ""
