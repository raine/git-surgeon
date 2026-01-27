from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def test_undo_partial_hunk(git_agent_exe, repo):
    """Undo only part of a large hunk using --lines."""
    (repo / "big.txt").write_text("top\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "initial")

    # Add 10 lines in a single contiguous block
    lines = ["top\n"] + [f"line{i}\n" for i in range(1, 11)]
    modify_file(repo, "big.txt", "".join(lines))
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add block")

    # Show hunk, get ID
    result = run_git_agent(git_agent_exe, repo, "hunks", "--commit", "HEAD")
    hunk_id = result.stdout.strip().split()[0]

    # Undo only lines 2-4 of the hunk (first 3 additions out of ~10)
    result = run_git_agent(git_agent_exe, repo, "undo", hunk_id, "--from", "HEAD", "--lines", "2-4")
    assert result.returncode == 0

    # Working tree should have those 3 lines removed but others intact
    content_lines = (repo / "big.txt").read_text().splitlines()
    assert "line1" not in content_lines  # undone
    assert "line2" not in content_lines  # undone
    assert "line3" not in content_lines  # undone
    assert "line4" in content_lines      # kept


def test_lines_rejects_multiple_ids(git_agent_exe, repo):
    """--lines requires exactly one hunk ID."""
    result = run_git_agent(git_agent_exe, repo, "undo", "abc", "def", "--from", "HEAD", "--lines", "1-5")
    assert result.returncode != 0


def test_lines_rejects_invalid_range(git_agent_exe, repo):
    """Invalid range format is rejected."""
    result = run_git_agent(git_agent_exe, repo, "undo", "abc", "--from", "HEAD", "--lines", "5-3")
    assert result.returncode != 0
