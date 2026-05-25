from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def test_unstage_single_hunk(git_agent_exe, repo):
    create_file(repo, "u.txt", "original\n")
    modify_file(repo, "u.txt", "modified\n")
    run_git(repo, "add", "u.txt")

    # Verify it's staged
    staged = run_git(repo, "diff", "--cached")
    assert "modified" in staged.stdout

    ids = _get_hunk_ids(git_agent_exe, repo, "--staged")
    assert len(ids) == 1

    result = run_git_agent(git_agent_exe, repo, "unstage", ids[0])
    assert result.returncode == 0
    assert ids[0] in result.stderr.splitlines()

    # Now staged should be empty
    staged = run_git(repo, "diff", "--cached")
    assert staged.stdout.strip() == ""

    # Unstaged should show the change
    unstaged = run_git(repo, "diff")
    assert "modified" in unstaged.stdout


def test_unstage_invalid_id(git_agent_exe, repo):
    result = run_git_agent(git_agent_exe, repo, "unstage", "invalid")
    assert result.returncode != 0


def test_unstage_atomic_failure_does_not_echo_matched_ids(git_agent_exe, repo):
    content = "a\n" + "mid\n" * 20 + "z\n"
    create_file(repo, "atomic.txt", content)
    modify_file(repo, "atomic.txt", "a1\n" + "mid\n" * 20 + "z1\n")
    run_git(repo, "add", "atomic.txt")

    ids = _get_hunk_ids(git_agent_exe, repo, "--staged")
    assert len(ids) >= 2

    result = run_git_agent(git_agent_exe, repo, "unstage", ids[0], "invalid")
    assert result.returncode != 0
    assert ids[0] not in result.stderr.splitlines()
    assert "not found" in result.stderr

    staged = run_git(repo, "diff", "--cached")
    assert "a1" in staged.stdout
    assert "z1" in staged.stdout


def test_unstage_inline_range_has_targeted_error(git_agent_exe, repo):
    create_file(repo, "inline.txt", "original\n")
    modify_file(repo, "inline.txt", "modified\n")
    run_git(repo, "add", "inline.txt")

    ids = _get_hunk_ids(git_agent_exe, repo, "--staged")
    assert len(ids) == 1

    result = run_git_agent(git_agent_exe, repo, "unstage", f"{ids[0]}:1-2")
    assert result.returncode != 0
    assert "inline ranges are not supported" in result.stderr
    assert "--lines" in result.stderr

    staged = run_git(repo, "diff", "--cached")
    assert "modified" in staged.stdout
