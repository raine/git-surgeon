from conftest import run_git_agent, run_git, create_file


def _commit_sha(repo, ref="HEAD"):
    result = run_git(repo, "rev-parse", ref)
    return result.stdout.strip()


def _commit_message(repo, ref="HEAD"):
    result = run_git(repo, "log", "-1", "--format=%B", ref)
    return result.stdout.strip()


def _commit_subjects(repo):
    """Return list of commit subjects oldest-first."""
    result = run_git(repo, "log", "--reverse", "--format=%s")
    return [s for s in result.stdout.strip().split("\n") if s]


def test_reword_head(git_agent_exe, repo):
    create_file(repo, "f.txt", "content\n")

    old_sha = _commit_sha(repo)
    result = run_git_agent(git_agent_exe, repo, "reword", "HEAD", "-m", "new message")
    assert result.returncode == 0

    # SHA should change (amended)
    new_sha = _commit_sha(repo)
    assert old_sha != new_sha

    # Message should be updated
    msg = _commit_message(repo)
    assert msg == "new message"


def test_reword_head_multiline(git_agent_exe, repo):
    create_file(repo, "f.txt", "content\n")

    result = run_git_agent(
        git_agent_exe, repo, "reword", "HEAD", "-m", "subject", "-m", "body paragraph"
    )
    assert result.returncode == 0

    msg = _commit_message(repo)
    assert "subject" in msg
    assert "body paragraph" in msg


def test_reword_earlier_commit(git_agent_exe, repo):
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)

    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")

    result = run_git_agent(
        git_agent_exe, repo, "reword", target_sha, "-m", "renamed commit"
    )
    assert result.returncode == 0

    # All commits should still exist
    subjects = _commit_subjects(repo)
    assert "renamed commit" in subjects
    assert "add b.txt" in subjects
    assert "add c.txt" in subjects


def test_reword_root_commit(git_agent_exe, repo):
    # The repo fixture has an init commit with .gitkeep as root
    root_sha = run_git(repo, "log", "--reverse", "--format=%H").stdout.strip().split("\n")[0]

    # Create a later commit so rebase has work to do
    create_file(repo, "later.txt", "later\n")

    result = run_git_agent(
        git_agent_exe, repo, "reword", root_sha, "-m", "new root message"
    )
    assert result.returncode == 0

    # Verify later commits still exist
    subjects = _commit_subjects(repo)
    assert "add later.txt" in subjects

    # Verify root commit has new message
    new_root_sha = run_git(repo, "log", "--reverse", "--format=%H").stdout.strip().split("\n")[0]
    msg = _commit_message(repo, new_root_sha)
    assert msg == "new root message"


def test_reword_invalid_commit(git_agent_exe, repo):
    result = run_git_agent(
        git_agent_exe, repo, "reword", "nonexistent", "-m", "message"
    )
    assert result.returncode != 0
