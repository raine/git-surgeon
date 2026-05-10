"""Tests for the fold command (fold a commit into an earlier commit)."""

from conftest import run_git_agent, run_git, create_file, modify_file


def _commit_sha(repo, ref="HEAD"):
    result = run_git(repo, "rev-parse", ref)
    return result.stdout.strip()


def _commit_subjects(repo):
    """Return list of commit subjects oldest-first."""
    result = run_git(repo, "log", "--reverse", "--format=%s")
    return [s for s in result.stdout.strip().split("\n") if s]


def test_fold_head_into_earlier(git_agent_exe, repo):
    """Fold HEAD into an earlier commit, preserving intermediate commits."""
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)

    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")

    # Create a fix commit at HEAD that should be folded into the target
    modify_file(repo, "a.txt", "aaa fixed\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix a.txt")

    result = run_git_agent(git_agent_exe, repo, "fold", target_sha)
    assert result.returncode == 0

    # Intermediate commits should still exist
    subjects = _commit_subjects(repo)
    assert "add a.txt" in subjects
    assert "add b.txt" in subjects
    assert "add c.txt" in subjects
    # The fix commit should be gone (folded)
    assert "fix a.txt" not in subjects

    # The target commit should now contain the fix
    result = run_git(repo, "log", "--all", "--format=%H %s")
    for line in result.stdout.strip().split("\n"):
        if "add a.txt" in line:
            sha = line.split()[0]
            break
    show = run_git(repo, "show", sha)
    assert "aaa fixed" in show.stdout


def test_fold_with_from_flag(git_agent_exe, repo):
    """Fold a non-HEAD commit into an earlier commit using --from."""
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)

    create_file(repo, "b.txt", "bbb\n")

    # Create the fix commit (not at HEAD)
    modify_file(repo, "a.txt", "aaa fixed\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix a.txt")
    fix_sha = _commit_sha(repo)

    # Add another commit after the fix so fix is not at HEAD
    create_file(repo, "c.txt", "ccc\n")

    result = run_git_agent(
        git_agent_exe, repo, "fold", target_sha, "--from", fix_sha
    )
    assert result.returncode == 0

    subjects = _commit_subjects(repo)
    assert "add a.txt" in subjects
    assert "add b.txt" in subjects
    assert "add c.txt" in subjects
    assert "fix a.txt" not in subjects


def test_fold_adjacent_commits(git_agent_exe, repo):
    """Fold HEAD into the commit immediately before it (HEAD~1)."""
    create_file(repo, "a.txt", "aaa\n")
    create_file(repo, "b.txt", "bbb\n")

    # Fix commit at HEAD
    modify_file(repo, "a.txt", "aaa fixed\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix a.txt")

    target_sha = _commit_sha(repo, "HEAD~1")
    result = run_git_agent(git_agent_exe, repo, "fold", target_sha)
    assert result.returncode == 0

    subjects = _commit_subjects(repo)
    assert "fix a.txt" not in subjects
    assert "add b.txt" in subjects


def test_fold_same_commit_errors(git_agent_exe, repo):
    """Fold with target == source should error."""
    create_file(repo, "a.txt", "aaa\n")

    result = run_git_agent(git_agent_exe, repo, "fold", "HEAD")
    assert result.returncode != 0
    assert "same commit" in result.stderr.lower()


def test_fold_not_ancestor_errors(git_agent_exe, repo):
    """Fold where target is not ancestor of source should error."""
    create_file(repo, "a.txt", "main\n")
    run_git(repo, "checkout", "-b", "other")
    create_file(repo, "b.txt", "other\n")
    other_sha = _commit_sha(repo)

    run_git(repo, "checkout", "main")
    create_file(repo, "c.txt", "main2\n")

    result = run_git_agent(git_agent_exe, repo, "fold", other_sha)
    assert result.returncode != 0
    assert "not an ancestor" in result.stderr.lower()


def test_fold_merge_commits_errors(git_agent_exe, repo):
    """Fold with merge commits in range should error."""
    create_file(repo, "a.txt", "main\n")
    target_sha = _commit_sha(repo)

    run_git(repo, "checkout", "-b", "feature")
    create_file(repo, "b.txt", "feature\n")

    run_git(repo, "checkout", "main")
    create_file(repo, "c.txt", "main2\n")
    run_git(repo, "merge", "feature", "-m", "merge feature")

    # Create a fix commit
    modify_file(repo, "a.txt", "main fixed\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix a.txt")

    result = run_git_agent(git_agent_exe, repo, "fold", target_sha)
    assert result.returncode != 0
    assert "merge" in result.stderr.lower()


def test_fold_preserves_dirty_worktree(git_agent_exe, repo):
    """Fold should autostash and restore dirty working tree."""
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)

    create_file(repo, "b.txt", "bbb\n")

    # Fix commit at HEAD
    modify_file(repo, "a.txt", "aaa fixed\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix a.txt")

    # Make working tree dirty
    modify_file(repo, "b.txt", "bbb modified\n")

    result = run_git_agent(git_agent_exe, repo, "fold", target_sha)
    assert result.returncode == 0

    # Dirty file should be restored
    assert (repo / "b.txt").read_text() == "bbb modified\n"


def test_fold_rejects_staged_changes(git_agent_exe, repo):
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)
    create_file(repo, "b.txt", "bbb\n")
    modify_file(repo, "a.txt", "aaa staged\n")
    run_git(repo, "add", "a.txt")
    old_head = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "fold", target_sha)

    assert result.returncode != 0
    assert "index has staged changes" in result.stderr
    assert "git-surgeon amend" in result.stderr
    assert _commit_sha(repo) == old_head
    assert "aaa staged" in run_git(repo, "diff", "--cached").stdout


def test_fold_explicit_head_rejects_staged_changes(git_agent_exe, repo):
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)
    create_file(repo, "b.txt", "bbb\n")
    modify_file(repo, "a.txt", "aaa staged\n")
    run_git(repo, "add", "a.txt")
    old_head = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "fold", target_sha, "--from", "HEAD")

    assert result.returncode != 0
    assert "index has staged changes" in result.stderr
    assert _commit_sha(repo) == old_head
    assert "aaa staged" in run_git(repo, "diff", "--cached").stdout


def test_fixup_command_is_removed(git_agent_exe, repo):
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)

    result = run_git_agent(git_agent_exe, repo, "fixup", target_sha)

    assert result.returncode != 0
    assert "unrecognized subcommand" in result.stderr.lower()


def test_fold_root_commit(git_agent_exe, repo):
    """Fold HEAD into the root commit."""
    # The repo fixture creates a root commit with .gitkeep
    root_sha = (
        run_git(repo, "log", "--reverse", "--format=%H").stdout.strip().split("\n")[0]
    )

    create_file(repo, "a.txt", "aaa\n")

    # Create a fix that belongs in root
    (repo / "root_extra.txt").write_text("added to root\n")
    run_git(repo, "add", "root_extra.txt")
    run_git(repo, "commit", "-m", "fix root")

    result = run_git_agent(git_agent_exe, repo, "fold", root_sha)
    assert result.returncode == 0

    subjects = _commit_subjects(repo)
    assert "fix root" not in subjects
    assert "add a.txt" in subjects

    # Verify root commit now contains the new file
    new_root_sha = (
        run_git(repo, "log", "--reverse", "--format=%H").stdout.strip().split("\n")[0]
    )
    show = run_git(repo, "show", "--stat", new_root_sha)
    assert "root_extra.txt" in show.stdout


def test_fold_multiple_from(git_agent_exe, repo):
    """Fold multiple non-HEAD commits into a target in one pass."""
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)

    # Interleave fix commits with unrelated commits
    modify_file(repo, "a.txt", "aaa fix1\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix1 a.txt")
    fix1_sha = _commit_sha(repo)

    create_file(repo, "b.txt", "bbb\n")  # unrelated

    modify_file(repo, "a.txt", "aaa fix1 fix2\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix2 a.txt")
    fix2_sha = _commit_sha(repo)

    create_file(repo, "c.txt", "ccc\n")  # unrelated

    modify_file(repo, "a.txt", "aaa fix1 fix2 fix3\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix3 a.txt")
    fix3_sha = _commit_sha(repo)

    create_file(repo, "d.txt", "ddd\n")  # unrelated, HEAD

    result = run_git_agent(
        git_agent_exe, repo, "fold", target_sha,
        "--from", fix1_sha, fix2_sha, fix3_sha
    )
    assert result.returncode == 0

    subjects = _commit_subjects(repo)
    # All three fix commits should be gone
    assert "fix1 a.txt" not in subjects
    assert "fix2 a.txt" not in subjects
    assert "fix3 a.txt" not in subjects
    # Unrelated commits should be preserved
    assert "add a.txt" in subjects
    assert "add b.txt" in subjects
    assert "add c.txt" in subjects
    assert "add d.txt" in subjects

    # Target commit should contain all fixes
    for line in run_git(repo, "log", "--all", "--format=%H %s").stdout.strip().split("\n"):
        if "add a.txt" in line:
            sha = line.split()[0]
            break
    show = run_git(repo, "show", sha)
    assert "aaa fix1 fix2 fix3" in show.stdout


def test_fold_multiple_from_preserves_dirty_worktree(git_agent_exe, repo):
    """Multi-source fold should autostash and restore dirty working tree."""
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)

    modify_file(repo, "a.txt", "aaa fix1\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix1 a.txt")
    fix1_sha = _commit_sha(repo)

    create_file(repo, "b.txt", "bbb\n")

    modify_file(repo, "a.txt", "aaa fix1 fix2\n")
    run_git(repo, "add", "a.txt")
    run_git(repo, "commit", "-m", "fix2 a.txt")
    fix2_sha = _commit_sha(repo)

    create_file(repo, "c.txt", "ccc\n")

    # Make working tree dirty
    modify_file(repo, "b.txt", "bbb modified\n")

    result = run_git_agent(
        git_agent_exe, repo, "fold", target_sha,
        "--from", fix1_sha, fix2_sha
    )
    assert result.returncode == 0

    # Dirty file should be restored
    assert (repo / "b.txt").read_text() == "bbb modified\n"

    subjects = _commit_subjects(repo)
    assert "fix1 a.txt" not in subjects
    assert "fix2 a.txt" not in subjects
    assert "add b.txt" in subjects
    assert "add c.txt" in subjects
