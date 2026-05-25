import subprocess

from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def test_commit_to_another_branch(git_agent_exe, repo):
    """Commit a hunk to a different branch without checking it out."""
    create_file(repo, "f.txt", "original\n")
    run_git(repo, "branch", "target")

    modify_file(repo, "f.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 1

    # Record current branch HEAD before
    main_sha_before = run_git(repo, "rev-parse", "HEAD").stdout.strip()

    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "cross-branch commit")
    assert result.returncode == 0, result.stderr
    assert ids[0] in result.stderr.splitlines()

    # Hunk should be discarded from working tree
    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() == ""

    # Current branch HEAD should not have moved
    main_sha_after = run_git(repo, "rev-parse", "HEAD").stdout.strip()
    assert main_sha_before == main_sha_after

    # Index should still be clean
    staged = run_git(repo, "diff", "--cached")
    assert staged.stdout.strip() == ""

    # Target branch should have the commit
    log = run_git(repo, "log", "target", "-1", "--format=%s")
    assert log.stdout.strip() == "cross-branch commit"

    # Verify the file content on target branch
    show = run_git(repo, "show", "target:f.txt")
    assert show.stdout.strip() == "modified"


def test_commit_to_message_body_starting_with_hyphen(git_agent_exe, repo):
    create_file(repo, "dash.txt", "original\n")
    run_git(repo, "branch", "target")

    modify_file(repo, "dash.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    result = run_git_agent(
        git_agent_exe,
        repo,
        "commit-to",
        "target",
        ids[0],
        "-m",
        "subject",
        "-m",
        "- body line",
    )
    assert result.returncode == 0, result.stderr

    log = run_git(repo, "log", "target", "-1", "--format=%B")
    assert log.stdout.strip() == "subject\n\n- body line"


def test_commit_to_subject_starting_with_hyphen(git_agent_exe, repo):
    create_file(repo, "dash-subject.txt", "original\n")
    run_git(repo, "branch", "target")

    modify_file(repo, "dash-subject.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    result = run_git_agent(
        git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "- subject"
    )
    assert result.returncode == 0, result.stderr

    log = run_git(repo, "log", "target", "-1", "--format=%B")
    assert log.stdout.strip() == "- subject"


def test_commit_to_with_inline_range(git_agent_exe, repo):
    """Commit partial hunk: selected lines go to target, unselected remain locally."""
    # Use a file where git produces interleaved -/+ pairs (single line change)
    # followed by context, so line ranges are predictable
    content = "aaa\nbbb\nccc\n"
    create_file(repo, "r.txt", content)
    run_git(repo, "branch", "target")

    modify_file(repo, "r.txt", "AAA\nbbb\nCCC\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) >= 1

    # Use show to see line numbers and pick the first change only
    show_result = run_git_agent(git_agent_exe, repo, "show", ids[0])
    lines = show_result.stdout.strip().split("\n")

    # Find the line range for -aaa/+AAA (first two change lines)
    first_minus = None
    first_plus_end = None
    for line in lines:
        # Lines look like "1:-aaa" or "2:+AAA"
        if ":" not in line:
            continue
        num_str = line.split(":")[0].strip()
        rest = line[line.index(":") + 1:]
        if first_minus is None and rest.startswith("-"):
            first_minus = int(num_str)
        if first_minus is not None and rest.startswith("+"):
            first_plus_end = int(num_str)
            break

    assert first_minus is not None and first_plus_end is not None

    result = run_git_agent(
        git_agent_exe, repo, "commit-to", "target",
        f"{ids[0]}:{first_minus}-{first_plus_end}", "-m", "partial"
    )
    assert result.returncode == 0, result.stderr

    log = run_git(repo, "log", "target", "-1", "--format=%s")
    assert log.stdout.strip() == "partial"

    # Target should have AAA but not CCC
    show = run_git(repo, "show", "target:r.txt")
    assert "AAA" in show.stdout
    assert "CCC" not in show.stdout

    # CCC change should remain locally
    unstaged = run_git(repo, "diff")
    assert "CCC" in unstaged.stdout


def test_commit_to_rejects_current_branch(git_agent_exe, repo):
    """Should refuse when target branch is the currently checked out branch."""
    branch = run_git(repo, "rev-parse", "--abbrev-ref", "HEAD")
    current = branch.stdout.strip()

    create_file(repo, "g.txt", "original\n")
    modify_file(repo, "g.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    result = run_git_agent(git_agent_exe, repo, "commit-to", current, ids[0], "-m", "nope")
    assert result.returncode != 0
    assert "currently checked out" in result.stderr


def test_commit_to_rejects_dirty_index(git_agent_exe, repo):
    """Should refuse if index has staged changes."""
    run_git(repo, "branch", "target")

    create_file(repo, "x.txt", "x\n")
    create_file(repo, "y.txt", "y\n")
    modify_file(repo, "x.txt", "x changed\n")
    modify_file(repo, "y.txt", "y changed\n")

    run_git(repo, "add", "x.txt")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) >= 1

    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "should fail")
    assert result.returncode != 0
    assert "staged changes" in result.stderr


def test_commit_to_preserves_other_changes(git_agent_exe, repo):
    """Committing one hunk to another branch should preserve other working tree changes."""
    create_file(repo, "a.txt", "a\n")
    create_file(repo, "b.txt", "b\n")
    run_git(repo, "branch", "target")

    modify_file(repo, "a.txt", "a changed\n")
    modify_file(repo, "b.txt", "b changed\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 2

    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "just one")
    assert result.returncode == 0, result.stderr

    # Should still have unstaged changes for the other file
    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() != ""


def test_commit_to_nonexistent_branch(git_agent_exe, repo):
    """Should fail when target branch does not exist."""
    create_file(repo, "h.txt", "original\n")
    modify_file(repo, "h.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    result = run_git_agent(git_agent_exe, repo, "commit-to", "nonexistent", ids[0], "-m", "nope")
    assert result.returncode != 0


def test_commit_to_multiple_m_flags(git_agent_exe, repo):
    """Multiple -m flags should be joined by blank lines."""
    create_file(repo, "mm.txt", "original\n")
    run_git(repo, "branch", "target")

    modify_file(repo, "mm.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "subject", "-m", "body text")
    assert result.returncode == 0, result.stderr

    log = run_git(repo, "log", "target", "-1", "--format=%B")
    body = log.stdout.strip()
    assert body.startswith("subject")
    assert "body text" in body


def test_commit_to_conflict_aborts_safely(git_agent_exe, repo):
    """If the patch conflicts with the target branch, nothing should be modified."""
    create_file(repo, "c.txt", "line1\nline2\nline3\n")
    run_git(repo, "branch", "target")

    # Diverge the target branch
    run_git(repo, "checkout", "target")
    modify_file(repo, "c.txt", "line1\nCONFLICT\nline3\n")
    run_git(repo, "add", "c.txt")
    run_git(repo, "commit", "-m", "target change")
    target_sha_before = run_git(repo, "rev-parse", "target").stdout.strip()

    # Back to main with a conflicting local change
    run_git(repo, "checkout", "main")
    modify_file(repo, "c.txt", "line1\nLOCAL_CHANGE\nline3\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "will fail")
    assert result.returncode != 0
    assert ids[0] not in result.stderr.splitlines()

    # Target branch HEAD should not have moved
    target_sha_after = run_git(repo, "rev-parse", "target").stdout.strip()
    assert target_sha_before == target_sha_after

    # Local changes should still be present
    unstaged = run_git(repo, "diff")
    assert "LOCAL_CHANGE" in unstaged.stdout

    # Index should be clean
    staged = run_git(repo, "diff", "--cached")
    assert staged.stdout.strip() == ""


def test_commit_to_same_file_multiple_hunks(git_agent_exe, repo):
    """Commit one hunk from a file to target, keep the other hunk locally."""
    content = "line1\n" + "mid\n" * 20 + "line_end\n"
    create_file(repo, "m.txt", content)
    run_git(repo, "branch", "target")

    new_content = "line1_changed\n" + "mid\n" * 20 + "line_end_changed\n"
    modify_file(repo, "m.txt", new_content)

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) >= 2

    # Commit only the first hunk
    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "first hunk only")
    assert result.returncode == 0, result.stderr

    # Target branch should have the first change
    show = run_git(repo, "show", "target:m.txt")
    assert "line1_changed" in show.stdout

    # The other hunk should remain as a local change
    unstaged = run_git(repo, "diff")
    assert "line_end_changed" in unstaged.stdout

    # Target should NOT have the second change
    assert "line_end_changed" not in show.stdout


def test_commit_to_file_deletion(git_agent_exe, repo):
    """Committing a deletion hunk should remove the file from the target branch."""
    create_file(repo, "delete_me.txt", "goodbye\n")
    run_git(repo, "branch", "target")

    # Delete the file locally
    (repo / "delete_me.txt").unlink()

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 1

    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "delete file")
    assert result.returncode == 0, result.stderr

    # File should no longer exist on target branch
    ls_tree = run_git(repo, "ls-tree", "-r", "target")
    assert "delete_me.txt" not in ls_tree.stdout

    # Working tree should remain clean (file already deleted, discard is a no-op)
    unstaged = run_git(repo, "diff")
    assert unstaged.stdout.strip() == ""


def test_commit_to_invalid_id(git_agent_exe, repo):
    """Should fail on an invalid hunk ID without leaving partial state."""
    run_git(repo, "branch", "target")
    target_sha_before = run_git(repo, "rev-parse", "target").stdout.strip()

    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", "invalid_id", "-m", "nope")
    assert result.returncode != 0
    assert "not found" in result.stderr.lower()

    # Target should not have moved
    target_sha_after = run_git(repo, "rev-parse", "target").stdout.strip()
    assert target_sha_before == target_sha_after


def test_commit_to_detached_head(git_agent_exe, repo):
    """Should work when HEAD is detached (not on any branch)."""
    create_file(repo, "d.txt", "original\n")
    run_git(repo, "branch", "target")

    # Detach HEAD
    head_sha = run_git(repo, "rev-parse", "HEAD").stdout.strip()
    run_git(repo, "checkout", "--detach", head_sha)

    modify_file(repo, "d.txt", "modified\n")

    ids = _get_hunk_ids(git_agent_exe, repo)
    assert len(ids) == 1

    result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "from detached")
    assert result.returncode == 0, result.stderr

    log = run_git(repo, "log", "target", "-1", "--format=%s")
    assert log.stdout.strip() == "from detached"


def test_commit_to_worktree(git_agent_exe, repo):
    """The motivating use case: commit to a branch checked out in another worktree."""
    create_file(repo, "w.txt", "original\n")
    run_git(repo, "branch", "target")

    # Create a worktree with 'target' checked out
    wt_path = repo.parent / "worktree-target"
    run_git(repo, "worktree", "add", str(wt_path), "target")

    try:
        # Make a local change on main
        modify_file(repo, "w.txt", "modified\n")

        ids = _get_hunk_ids(git_agent_exe, repo)
        assert len(ids) == 1

        # Commit to target even though it's checked out in another worktree
        result = run_git_agent(git_agent_exe, repo, "commit-to", "target", ids[0], "-m", "cross-worktree commit")
        assert result.returncode == 0, result.stderr

        # Target branch should have the commit
        log = run_git(repo, "log", "target", "-1", "--format=%s")
        assert log.stdout.strip() == "cross-worktree commit"

        show = run_git(repo, "show", "target:w.txt")
        assert show.stdout.strip() == "modified"

        # Local change should be discarded
        unstaged = run_git(repo, "diff")
        assert unstaged.stdout.strip() == ""
    finally:
        # Clean up worktree
        subprocess.run(["git", "worktree", "remove", str(wt_path)], cwd=repo)
