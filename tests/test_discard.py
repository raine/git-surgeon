from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def test_discard_single_hunk(git_agent_exe, repo):
    create_file(repo, "d.txt", "original\n")
    modify_file(repo, "d.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 1

    result = run_git_agent(git_agent_exe, repo, "discard", ids[0])
    assert result.returncode == 0

    # File should be back to original
    content = (repo / "d.txt").read_text()
    assert content == "original\n"

    # No diff
    diff = run_git(repo, "diff")
    assert diff.stdout.strip() == ""


def test_discard_one_of_two_hunks(git_agent_exe, repo):
    content = "line1\n" + "mid\n" * 20 + "line_end\n"
    create_file(repo, "d2.txt", content)
    new_content = "line1_changed\n" + "mid\n" * 20 + "line_end_changed\n"
    modify_file(repo, "d2.txt", new_content)

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) >= 2

    # Discard only the first hunk
    result = run_git_agent(git_agent_exe, repo, "discard", ids[0])
    assert result.returncode == 0

    # Should still have changes (the other hunk)
    diff = run_git(repo, "diff")
    assert diff.stdout.strip() != ""
