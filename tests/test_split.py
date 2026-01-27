from conftest import run_git_agent, run_git, create_file, modify_file


def _get_hunk_ids(exe, repo, *extra_args):
    result = run_git_agent(exe, repo, "hunks", *extra_args)
    ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith("  "):
            ids.append(line.split()[0])
    return ids


def _commit_subjects(repo):
    result = run_git(repo, "log", "--reverse", "--format=%s")
    return [s for s in result.stdout.strip().split("\n") if s]


def _commit_sha(repo, ref="HEAD"):
    result = run_git(repo, "rev-parse", ref)
    return result.stdout.strip()


def test_split_head_two_commits(git_agent_exe, repo):
    """Split HEAD into two commits by picking specific hunks."""
    # Create a file with two separate regions
    content = "top\n" + "ctx\n" * 20 + "bottom\n"
    create_file(repo, "f.txt", content)

    # Make a commit that modifies both regions
    new_content = "top modified\n" + "ctx\n" * 20 + "bottom modified\n"
    modify_file(repo, "f.txt", new_content)
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "modify both regions")

    # Get hunk IDs from the commit we want to split
    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 2

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", ids[0], "--message", "modify top",
        "--rest-message", "modify bottom",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "modify top" in subjects
    assert "modify bottom" in subjects
    assert "modify both regions" not in subjects


def test_split_head_picks_correct_hunks(git_agent_exe, repo):
    """Verify the picked hunks end up in the right commits."""
    content = "aaa\n" + "ctx\n" * 20 + "bbb\n"
    create_file(repo, "f.txt", content)

    new_content = "aaa changed\n" + "ctx\n" * 20 + "bbb changed\n"
    modify_file(repo, "f.txt", new_content)
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "change both")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 2

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", ids[0], "--message", "change top",
        "--rest-message", "change bottom",
    )
    assert result.returncode == 0, result.stderr

    # Check first picked commit has the top change
    log = run_git(repo, "log", "--all", "--format=%H %s")
    for line in log.stdout.strip().split("\n"):
        if "change top" in line:
            sha = line.split()[0]
            show = run_git(repo, "show", sha)
            assert "aaa changed" in show.stdout
            assert "bbb changed" not in show.stdout
        elif "change bottom" in line:
            sha = line.split()[0]
            show = run_git(repo, "show", sha)
            assert "bbb changed" in show.stdout
            assert "aaa changed" not in show.stdout


def test_split_head_multiple_files(git_agent_exe, repo):
    """Split a commit that touches multiple files."""
    create_file(repo, "a.txt", "aaa\n")
    create_file(repo, "b.txt", "bbb\n")

    modify_file(repo, "a.txt", "aaa modified\n")
    modify_file(repo, "b.txt", "bbb modified\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify both files")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 2

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", ids[0], "--message", "modify a",
        "--rest-message", "modify b",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "modify a" in subjects
    assert "modify b" in subjects


def test_split_head_rest_message_defaults_to_original(git_agent_exe, repo):
    """Without --rest-message, use the original commit message."""
    content = "top\n" + "ctx\n" * 20 + "bottom\n"
    create_file(repo, "f.txt", content)

    new_content = "top modified\n" + "ctx\n" * 20 + "bottom modified\n"
    modify_file(repo, "f.txt", new_content)
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "original message")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", ids[0], "--message", "picked part",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "picked part" in subjects
    assert "original message" in subjects


def test_split_head_multiple_pick_groups(git_agent_exe, repo):
    """Split into three commits using multiple --pick groups."""
    create_file(repo, "a.txt", "aaa\n")
    create_file(repo, "b.txt", "bbb\n")
    create_file(repo, "c.txt", "ccc\n")

    modify_file(repo, "a.txt", "aaa modified\n")
    modify_file(repo, "b.txt", "bbb modified\n")
    modify_file(repo, "c.txt", "ccc modified\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify all three")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 3

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", ids[0], "--message", "modify first",
        "--pick", ids[1], "--message", "modify second",
        "--rest-message", "modify third",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "modify first" in subjects
    assert "modify second" in subjects
    assert "modify third" in subjects


def test_split_earlier_commit(git_agent_exe, repo):
    """Split a non-HEAD commit using rebase."""
    content = "top\n" + "ctx\n" * 20 + "bottom\n"
    create_file(repo, "f.txt", content)

    # Create the commit we want to split
    new_content = "top modified\n" + "ctx\n" * 20 + "bottom modified\n"
    modify_file(repo, "f.txt", new_content)
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "modify both regions")
    target_sha = _commit_sha(repo)

    # Create a later commit
    create_file(repo, "later.txt", "later\n")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", target_sha)
    assert len(ids) == 2

    result = run_git_agent(
        git_agent_exe, repo, "split", target_sha,
        "--pick", ids[0], "--message", "modify top",
        "--rest-message", "modify bottom",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "modify top" in subjects
    assert "modify bottom" in subjects
    assert "add later.txt" in subjects
    assert "modify both regions" not in subjects


def test_split_with_line_ranges(git_agent_exe, repo):
    """Split with inline id:range syntax for partial hunk selection."""
    # Create a file with a single hunk that has multiple changes
    lines = ["line{}\n".format(i) for i in range(1, 11)]
    create_file(repo, "f.txt", "".join(lines))

    # Modify several lines in a single hunk
    new_lines = list(lines)
    new_lines[1] = "line2 modified\n"
    new_lines[3] = "line4 modified\n"
    new_lines[7] = "line8 modified\n"
    modify_file(repo, "f.txt", "".join(new_lines))
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "modify several lines")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) >= 1

    # Show the hunk to understand line numbering
    show = run_git_agent(git_agent_exe, repo, "show", ids[0], "--commit", "HEAD")

    # Pick first few lines of the hunk
    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", f"{ids[0]}:1-6", "--message", "first changes",
        "--rest-message", "remaining changes",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "first changes" in subjects
    assert "remaining changes" in subjects


def test_split_multiple_ids_single_group(git_agent_exe, repo):
    """Pick multiple hunks into a single commit."""
    create_file(repo, "a.txt", "a\n")
    create_file(repo, "b.txt", "b\n")
    create_file(repo, "c.txt", "c\n")

    modify_file(repo, "a.txt", "a mod\n")
    modify_file(repo, "b.txt", "b mod\n")
    modify_file(repo, "c.txt", "c mod\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "modify three files")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 3

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", ids[0], ids[1], "--message", "modify a and b",
        "--rest-message", "modify c",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "modify a and b" in subjects
    assert "modify c" in subjects
    assert "modify three files" not in subjects

    # Verify the combined commit has both changes
    log = run_git(repo, "log", "--all", "--format=%H %s")
    for line in log.stdout.strip().split("\n"):
        if "modify a and b" in line:
            sha = line.split()[0]
            show = run_git(repo, "show", sha)
            assert "a mod" in show.stdout
            assert "b mod" in show.stdout
            assert "c mod" not in show.stdout


def test_split_dirty_working_tree_fails(git_agent_exe, repo):
    """Ensure split aborts if working tree is dirty."""
    create_file(repo, "f.txt", "content\n")

    modify_file(repo, "f.txt", "dirty\n")

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", "1234567", "--message", "wont happen",
    )
    assert result.returncode != 0
    assert "dirty" in result.stderr


def test_split_picks_all_hunks_no_rest_commit(git_agent_exe, repo):
    """If all hunks are picked, no rest commit should be created."""
    create_file(repo, "a.txt", "a\n")
    create_file(repo, "b.txt", "b\n")

    modify_file(repo, "a.txt", "a mod\n")
    modify_file(repo, "b.txt", "b mod\n")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "original")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 2

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", ids[0], "--message", "commit one",
        "--pick", ids[1], "--message", "commit two",
        "--rest-message", "should not exist",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "commit one" in subjects
    assert "commit two" in subjects
    assert "should not exist" not in subjects


def test_split_no_pick_fails(git_agent_exe, repo):
    """Error when no --pick is provided."""
    result = run_git_agent(git_agent_exe, repo, "split", "HEAD")
    assert result.returncode != 0


def test_split_invalid_hunk_id(git_agent_exe, repo):
    """Error when a picked hunk ID doesn't exist in the commit."""
    content = "hello\n"
    create_file(repo, "f.txt", content)
    modify_file(repo, "f.txt", "changed\n")
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "a change")

    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", "0000000", "--message", "nope",
    )
    assert result.returncode != 0
    assert "not found" in result.stderr


def test_split_pick_missing_message(git_agent_exe, repo):
    """Error when --pick has no corresponding --message."""
    result = run_git_agent(
        git_agent_exe, repo, "split", "HEAD",
        "--pick", "abc1234",
    )
    assert result.returncode != 0
