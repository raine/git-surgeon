from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    """Extract hunk IDs from hunks output."""
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def test_stage_single_hunk(git_agent_exe, repo):
    create_file(repo, "s.txt", "original\n")
    modify_file(repo, "s.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 1

    result = run_git_agent(git_agent_exe, repo, "stage", ids[0])
    assert result.returncode == 0

    # Staged diff should show the change
    staged = run_git(repo, "diff", "--cached")
    assert "modified" in staged.stdout

    # Unstaged diff should be empty
    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() == ""


def test_stage_multiple_hunks_same_file(git_agent_exe, repo):
    content = "line1\n" + "mid\n" * 20 + "line_end\n"
    create_file(repo, "m.txt", content)
    new_content = "line1_changed\n" + "mid\n" * 20 + "line_end_changed\n"
    modify_file(repo, "m.txt", new_content)

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) >= 2

    result = run_git_agent(git_agent_exe, repo, "stage", *ids)
    assert result.returncode == 0

    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() == ""


def test_stage_hunks_different_files(git_agent_exe, repo):
    create_file(repo, "f1.txt", "f1\n")
    create_file(repo, "f2.txt", "f2\n")
    modify_file(repo, "f1.txt", "f1 changed\n")
    modify_file(repo, "f2.txt", "f2 changed\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 2

    result = run_git_agent(git_agent_exe, repo, "stage", *ids)
    assert result.returncode == 0

    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() == ""


def test_stage_invalid_id(git_agent_exe, repo):
    result = run_git_agent(git_agent_exe, repo, "stage", "invalid")
    assert result.returncode != 0
