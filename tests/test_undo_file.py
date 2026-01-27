from conftest import run_git_agent, run_git, modify_file


def test_undo_file_single(repo, git_agent_exe):
    """Undo all changes to a single file from a commit."""
    (repo / "file.txt").write_text("line1\nline2\nline3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add file")

    modify_file(repo, "file.txt", "changed1\nchanged2\nchanged3\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify all lines")

    result = run_git_agent(git_agent_exe, repo, "undo-file", "file.txt", "--from", "HEAD")
    assert result.returncode == 0

    diff = run_git(repo, "diff")
    assert "+line1" in diff.stdout
    assert "+line2" in diff.stdout
    assert "+line3" in diff.stdout


def test_undo_file_one_of_multiple(repo, git_agent_exe):
    """Undo one file when the commit touched multiple files."""
    (repo / "a.txt").write_text("aaa\n")
    (repo / "b.txt").write_text("bbb\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add files")

    modify_file(repo, "a.txt", "AAA\n")
    modify_file(repo, "b.txt", "BBB\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify both")

    result = run_git_agent(git_agent_exe, repo, "undo-file", "a.txt", "--from", "HEAD")
    assert result.returncode == 0

    diff = run_git(repo, "diff")
    # a.txt should be reverted
    assert "+aaa" in diff.stdout
    # b.txt should be untouched
    assert "bbb" not in diff.stdout


def test_undo_file_not_in_commit(repo, git_agent_exe):
    """Undo fails when file wasn't changed in the commit."""
    (repo / "file.txt").write_text("hello\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add file")

    modify_file(repo, "file.txt", "world\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify")

    result = run_git_agent(git_agent_exe, repo, "undo-file", "nonexistent.txt", "--from", "HEAD")
    assert result.returncode != 0
