from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def test_show_hunk_content(git_agent_exe, repo):
    create_file(repo, "show.txt", "before\n")
    modify_file(repo, "show.txt", "after\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 1

    result = run_git_agent(git_agent_exe, repo, "show", ids[0])
    assert result.returncode == 0
    assert "@@" in result.stdout
    assert "-before" in result.stdout
    assert "+after" in result.stdout


def test_show_invalid_id(git_agent_exe, repo):
    result = run_git_agent(git_agent_exe, repo, "show", "invalid")
    assert result.returncode != 0
