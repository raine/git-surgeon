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


def test_undo_file_deletion(repo, git_agent_exe):
    """Undo a file deletion from a commit, recreating the file."""
    (repo / "deleted.txt").write_text("precious content\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add file")

    run_git(repo, "rm", "deleted.txt")
    run_git(repo, "commit", "-m", "delete file")

    result = run_git_agent(git_agent_exe, repo, "undo-file", "deleted.txt", "--from", "HEAD")
    assert result.returncode == 0

    # The deleted file should reappear as an unstaged change
    assert (repo / "deleted.txt").read_text() == "precious content\n"


def test_undo_file_deletion_lists_correct_path(repo, git_agent_exe):
    """Hunks listing shows the real file path for deletions, not /dev/null."""
    (repo / "gone.txt").write_text("data\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add file")

    run_git(repo, "rm", "gone.txt")
    run_git(repo, "commit", "-m", "delete file")

    result = run_git_agent(git_agent_exe, repo, "hunks", "--commit", "HEAD")
    assert result.returncode == 0
    assert "gone.txt" in result.stdout
    assert "dev/null" not in result.stdout
