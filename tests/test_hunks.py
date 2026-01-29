from conftest import run_git_agent, run_git, create_file, modify_file


def test_no_changes_empty_output(git_agent_exe, repo):
    result = run_git_agent(git_agent_exe, repo, "hunks")
    assert result.returncode == 0
    assert result.stdout == ""


def test_single_file_single_hunk(git_agent_exe, repo):
    create_file(repo, "hello.txt", "hello\n")
    modify_file(repo, "hello.txt", "hello world\n")

    result = run_git_agent(git_agent_exe, repo, "hunks")
    assert result.returncode == 0
    assert "hello.txt" in result.stdout
    # Should have a 7-char hex ID
    lines = result.stdout.strip().split("\n")
    first_line = lines[0]
    hunk_id = first_line.split()[0]
    assert len(hunk_id) == 7


def test_multiple_hunks_same_file(git_agent_exe, repo):
    # Create a file with two distant regions
    content = "line1\n" + "mid\n" * 20 + "line_end\n"
    create_file(repo, "multi.txt", content)
    new_content = "line1_changed\n" + "mid\n" * 20 + "line_end_changed\n"
    modify_file(repo, "multi.txt", new_content)

    result = run_git_agent(git_agent_exe, repo, "hunks")
    assert result.returncode == 0
    # Count hunk header lines (lines starting with a hex ID)
    header_lines = [l for l in result.stdout.strip().split("\n") if l and not l.startswith("  ")]
    assert len(header_lines) >= 2


def test_multiple_files(git_agent_exe, repo):
    create_file(repo, "a.txt", "a\n")
    create_file(repo, "b.txt", "b\n")
    modify_file(repo, "a.txt", "a changed\n")
    modify_file(repo, "b.txt", "b changed\n")

    result = run_git_agent(git_agent_exe, repo, "hunks")
    assert result.returncode == 0
    assert "a.txt" in result.stdout
    assert "b.txt" in result.stdout


def test_staged_flag(git_agent_exe, repo):
    create_file(repo, "staged.txt", "original\n")
    modify_file(repo, "staged.txt", "modified\n")
    run_git(repo, "add", "staged.txt")

    # --staged should show the hunk
    result = run_git_agent(git_agent_exe, repo, "hunks", "--staged")
    assert result.returncode == 0
    assert "staged.txt" in result.stdout

    # unstaged should be empty (all changes are staged)
    result = run_git_agent(git_agent_exe, repo, "hunks")
    assert result.returncode == 0
    assert "staged.txt" not in result.stdout


def test_file_filter(git_agent_exe, repo):
    create_file(repo, "x.txt", "x\n")
    create_file(repo, "y.txt", "y\n")
    modify_file(repo, "x.txt", "x changed\n")
    modify_file(repo, "y.txt", "y changed\n")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--file=x.txt")
    assert result.returncode == 0
    assert "x.txt" in result.stdout
    assert "y.txt" not in result.stdout


def test_hunk_id_stability(git_agent_exe, repo):
    create_file(repo, "stable.txt", "stable\n")
    modify_file(repo, "stable.txt", "changed\n")

    r1 = run_git_agent(git_agent_exe, repo, "hunks")
    r2 = run_git_agent(git_agent_exe, repo, "hunks")
    assert r1.stdout == r2.stdout


def test_additions_deletions_count(git_agent_exe, repo):
    create_file(repo, "count.txt", "old line\n")
    modify_file(repo, "count.txt", "new line\n")

    result = run_git_agent(git_agent_exe, repo, "hunks")
    assert result.returncode == 0
    assert "(+1 -1)" in result.stdout


def test_full_flag_shows_line_numbers(git_agent_exe, repo):
    create_file(repo, "full.txt", "line1\nline2\nline3\n")
    modify_file(repo, "full.txt", "line1\nchanged\nline3\n")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--full")
    assert result.returncode == 0
    # Should have line numbers like "1:", "2:", etc.
    assert "1:" in result.stdout
    # Should include context lines (not just changed lines)
    assert "line1" in result.stdout or "line3" in result.stdout


def test_full_flag_with_commit(git_agent_exe, repo):
    create_file(repo, "commit.txt", "original\n")
    run_git(repo, "add", "commit.txt")
    run_git(repo, "commit", "-m", "add file")
    modify_file(repo, "commit.txt", "modified\n")
    run_git(repo, "add", "commit.txt")
    run_git(repo, "commit", "-m", "modify file")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--commit=HEAD", "--full")
    assert result.returncode == 0
    assert "commit.txt" in result.stdout
    # Should have line numbers
    assert "1:" in result.stdout
