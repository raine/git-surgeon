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

    # Now staged should be empty
    staged = run_git(repo, "diff", "--cached")
    assert staged.stdout.strip() == ""

    # Unstaged should show the change
    unstaged = run_git(repo, "diff")
    assert "modified" in unstaged.stdout


def test_unstage_invalid_id(git_agent_exe, repo):
    result = run_git_agent(git_agent_exe, repo, "unstage", "invalid")
    assert result.returncode != 0
