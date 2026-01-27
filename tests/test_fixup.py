from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def _commit_sha(repo, ref="HEAD"):
    result = run_git(repo, "rev-parse", ref)
    return result.stdout.strip()


def _commit_subjects(repo):
    """Return list of commit subjects oldest-first."""
    result = run_git(repo, "log", "--reverse", "--format=%s")
    return [s for s in result.stdout.strip().split("\n") if s]


def test_fixup_head(git_agent_exe, repo):
    create_file(repo, "f.txt", "original\n")
    modify_file(repo, "f.txt", "modified\n")
    run_git(repo, "add", "f.txt")

    old_sha = _commit_sha(repo)
    result = run_git_agent(git_agent_exe, repo, "fixup", "HEAD")
    assert result.returncode == 0

    # SHA should change (amended)
    new_sha = _commit_sha(repo)
    assert old_sha != new_sha

    # The amended commit should contain the modification
    show = run_git(repo, "show", "--stat", "HEAD")
    assert "f.txt" in show.stdout


def test_fixup_earlier_commit(git_agent_exe, repo):
    create_file(repo, "a.txt", "aaa\n")
    target_sha = _commit_sha(repo)

    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")

    # Stage a change to fixup into the first file's commit
    modify_file(repo, "a.txt", "aaa modified\n")
    run_git(repo, "add", "a.txt")

    result = run_git_agent(git_agent_exe, repo, "fixup", target_sha)
    assert result.returncode == 0

    # All 3 original commits should still exist (plus init)
    subjects = _commit_subjects(repo)
    assert "add a.txt" in subjects
    assert "add b.txt" in subjects
    assert "add c.txt" in subjects

    # The target commit should now contain the modification
    # Find new sha of "add a.txt" commit
    result = run_git(repo, "log", "--all", "--format=%H %s")
    for line in result.stdout.strip().split("\n"):
        if "add a.txt" in line:
            sha = line.split()[0]
            break
    show = run_git(repo, "show", sha)
    assert "aaa modified" in show.stdout


def test_fixup_no_staged_changes(git_agent_exe, repo):
    result = run_git_agent(git_agent_exe, repo, "fixup", "HEAD")
    assert result.returncode != 0
    assert "no staged changes" in result.stderr


def test_fixup_preserves_unstaged_changes(git_agent_exe, repo):
    # Create a file with two regions separated by enough context
    content = "top\n" + "ctx\n" * 20 + "bottom\n"
    create_file(repo, "f.txt", content)

    # Create another commit so we have something to fixup into
    create_file(repo, "other.txt", "other\n")
    target_sha = _commit_sha(repo)

    # Modify both regions
    new_content = "top modified\n" + "ctx\n" * 20 + "bottom modified\n"
    modify_file(repo, "f.txt", new_content)

    # Stage only the first hunk
    ids = _get_hunk_ids(git_agent_exe, repo, "--file", "f.txt")
    assert len(ids) >= 2
    run_git_agent(git_agent_exe, repo, "stage", ids[0])

    result = run_git_agent(git_agent_exe, repo, "fixup", target_sha)
    assert result.returncode == 0

    # Unstaged change should survive
    diff = run_git(repo, "diff")
    assert "bottom modified" in diff.stdout or "top modified" in diff.stdout


def test_fixup_root_commit(git_agent_exe, repo):
    # The repo fixture has an init commit with .gitkeep as root
    root_sha = run_git(repo, "log", "--reverse", "--format=%H").stdout.strip().split("\n")[0]

    # Create a later commit so rebase has work to do
    create_file(repo, "later.txt", "later\n")

    # Stage a new file to fold into the root commit
    (repo / "root_extra.txt").write_text("added to root\n")
    run_git(repo, "add", "root_extra.txt")

    result = run_git_agent(git_agent_exe, repo, "fixup", root_sha)
    assert result.returncode == 0

    # Verify later commits still exist
    subjects = _commit_subjects(repo)
    assert "add later.txt" in subjects

    # Verify root commit now contains the new file
    new_root_sha = run_git(repo, "log", "--reverse", "--format=%H").stdout.strip().split("\n")[0]
    show = run_git(repo, "show", "--stat", new_root_sha)
    assert "root_extra.txt" in show.stdout
