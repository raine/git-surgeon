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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        "--message",
        "modify top",
        "--rest-message",
        "modify bottom",
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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        "--message",
        "change top",
        "--rest-message",
        "change bottom",
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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        "--message",
        "modify a",
        "--rest-message",
        "modify b",
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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        "--message",
        "picked part",
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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        "--message",
        "modify first",
        "--pick",
        ids[1],
        "--message",
        "modify second",
        "--rest-message",
        "modify third",
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
        git_agent_exe,
        repo,
        "split",
        target_sha,
        "--pick",
        ids[0],
        "--message",
        "modify top",
        "--rest-message",
        "modify bottom",
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

    # Pick first few lines of the hunk
    result = run_git_agent(
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        f"{ids[0]}:1-6",
        "--message",
        "first changes",
        "--rest-message",
        "remaining changes",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "first changes" in subjects
    assert "remaining changes" in subjects


def test_split_with_single_line_range(git_agent_exe, repo):
    """Split with inline id:N syntax for a single line."""
    lines = ["line{}\n".format(i) for i in range(1, 11)]
    create_file(repo, "f.txt", "".join(lines))

    new_lines = list(lines)
    new_lines[1] = "line2 modified\n"
    new_lines[3] = "line4 modified\n"
    modify_file(repo, "f.txt", "".join(new_lines))
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "modify two lines")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) >= 1

    # Use single-line syntax id:N (no dash)
    result = run_git_agent(
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        f"{ids[0]}:2",
        "--message",
        "first line change",
        "--rest-message",
        "second line change",
    )
    assert result.returncode == 0, result.stderr

    subjects = _commit_subjects(repo)
    assert "first line change" in subjects
    assert "second line change" in subjects


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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        ids[1],
        "--message",
        "modify a and b",
        "--rest-message",
        "modify c",
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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        "1234567",
        "--message",
        "wont happen",
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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        "--message",
        "commit one",
        "--pick",
        ids[1],
        "--message",
        "commit two",
        "--rest-message",
        "should not exist",
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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        "0000000",
        "--message",
        "nope",
    )
    assert result.returncode != 0
    assert "not found" in result.stderr


def test_split_same_hunk_multiple_ranges(git_agent_exe, repo):
    """Split with same hunk ID repeated for non-contiguous line ranges."""
    lines = ["line{}\n".format(i) for i in range(1, 11)]
    create_file(repo, "f.txt", "".join(lines))

    new_lines = list(lines)
    new_lines[1] = "line2 ALPHA\n"
    new_lines[3] = "line4 ALPHA\n"
    new_lines[7] = "line8 BETA\n"
    new_lines[9] = "line10 BETA\n"
    modify_file(repo, "f.txt", "".join(new_lines))
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "modify top and bottom")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 1
    hunk_id = ids[0]

    # Same hunk ID with two non-contiguous ranges in one --pick group
    result = run_git_agent(
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        f"{hunk_id}:2-5",
        f"{hunk_id}:9-12",
        "--message",
        "alpha and beta changes",
    )
    assert result.returncode == 0, f"split failed: {result.stderr}"

    subjects = _commit_subjects(repo)
    assert "alpha and beta changes" in subjects


def test_split_multi_pick_groups_with_line_ranges(git_agent_exe, repo):
    """Split one hunk across multiple --pick groups using line ranges.

    Reproduces the scenario where an AI agent splits a mixed-concern commit
    (logging + filtering + pagination in a single file) into separate commits.
    The second --pick group fails because hunk IDs are recomputed after the
    first group is committed, invalidating the original IDs.
    """
    create_file(
        repo,
        "app.py",
        "from flask import Flask, jsonify\n"
        "app = Flask(__name__)\n"
        "users = [\n"
        '    {"id": 1, "name": "Alice"},\n'
        '    {"id": 2, "name": "Bob"},\n'
        "]\n"
        "@app.route('/users')\n"
        "def list_users():\n"
        "    return jsonify(users)\n"
        "@app.route('/health')\n"
        "def health():\n"
        '    return jsonify({"status": "ok"})\n',
    )

    modify_file(
        repo,
        "app.py",
        "from flask import Flask, jsonify, request\n"
        "import logging\n"
        "app = Flask(__name__)\n"
        "logger = logging.getLogger(__name__)\n"
        "users = [\n"
        '    {"id": 1, "name": "Alice", "active": True},\n'
        '    {"id": 2, "name": "Bob", "active": True},\n'
        "]\n"
        "@app.route('/users')\n"
        "def list_users():\n"
        '    page = request.args.get("page", 1, type=int)\n'
        '    active = [u for u in users if u["active"]]\n'
        "    return jsonify(active[page:page+10])\n"
        "@app.route('/health')\n"
        "def health():\n"
        '    logger.info("health check")\n'
        '    return jsonify({"status": "ok"})\n',
    )
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add logging, filtering, and pagination")

    # Single hunk covering the whole diff. Show output:
    #  1:-from flask import Flask, jsonify
    #  2:+from flask import Flask, jsonify, request
    #  3:+import logging
    #  4: app = Flask(__name__)
    #  5:+logger = logging.getLogger(__name__)
    #  6: users = [
    #  7:-    {"id": 1, "name": "Alice"},
    #  8:-    {"id": 2, "name": "Bob"},
    #  9:+    {"id": 1, "name": "Alice", "active": True},
    # 10:+    {"id": 2, "name": "Bob", "active": True},
    # 11: ]
    # 12: @app.route('/users')
    # 13: def list_users():
    # 14:-    return jsonify(users)
    # 15:+    page = request.args.get("page", 1, type=int)
    # 16:+    active = [u for u in users if u["active"]]
    # 17:+    return jsonify(active[page:page+10])
    # 18: @app.route('/health')
    # 19: def health():
    # 20:+    logger.info("health check")
    # 21:     return jsonify({"status": "ok"})
    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 1
    hid = ids[0]

    # Split into 3 commits using line ranges from the same hunk:
    # - logging: lines 1-3,5,20
    # - filtering: lines 7-10,16
    # - pagination (rest): lines 14-15,17
    result = run_git_agent(
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        f"{hid}:1-3",
        f"{hid}:5",
        f"{hid}:20",
        "--message",
        "add logging",
        "--pick",
        f"{hid}:7-10",
        f"{hid}:16",
        "--message",
        "add filtering",
        "--rest-message",
        "add pagination",
    )
    assert result.returncode == 0, f"split failed: {result.stderr}"

    subjects = _commit_subjects(repo)
    assert "add logging" in subjects
    assert "add filtering" in subjects
    assert "add pagination" in subjects

    # Verify commit contents: check that added lines (+) land in the right commit
    def added_lines(sha):
        show = run_git(repo, "show", sha)
        return [l[1:] for l in show.stdout.split("\n") if l.startswith("+") and not l.startswith("+++")]

    log = run_git(repo, "log", "--all", "--format=%H %s")
    for line in log.stdout.strip().split("\n"):
        if "add logging" in line:
            lines = added_lines(line.split()[0])
            assert any("import logging" in l for l in lines)
            assert any("logger" in l for l in lines)
            assert not any('"active"' in l for l in lines)
            assert not any("page" in l for l in lines)
        elif "add filtering" in line:
            lines = added_lines(line.split()[0])
            assert any('"active"' in l for l in lines)
            assert not any("import logging" in l for l in lines)
            assert not any("page" in l for l in lines)
        elif "add pagination" in line:
            lines = added_lines(line.split()[0])
            assert any("page" in l for l in lines)
            assert not any("import logging" in l for l in lines)


def test_split_pick_missing_message(git_agent_exe, repo):
    """Error when --pick has no corresponding --message."""
    result = run_git_agent(
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        "abc1234",
    )
    assert result.returncode != 0


def test_split_picks_second_hunk_in_same_file(git_agent_exe, repo):
    """Picking the second hunk in a file should stage that hunk, not the first.

    This test exposes a bug where split remaps hunks by file path only,
    causing the wrong hunk to be staged when multiple hunks exist in one file.
    """
    # Create a file with two separate regions (enough context lines to create 2 hunks)
    content = "TOP ORIGINAL\n" + "ctx\n" * 20 + "BOTTOM ORIGINAL\n"
    create_file(repo, "f.txt", content)

    # Modify both regions to create two hunks
    new_content = "TOP MODIFIED\n" + "ctx\n" * 20 + "BOTTOM MODIFIED\n"
    modify_file(repo, "f.txt", new_content)
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "modify both regions")

    # Get hunk IDs - should be 2 hunks in the same file
    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")
    assert len(ids) == 2, f"Expected 2 hunks, got {len(ids)}"

    # Pick the SECOND hunk specifically (the bottom change)
    result = run_git_agent(
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[1],  # Pick second hunk, not first
        "--message",
        "modify bottom only",
        "--rest-message",
        "modify top only",
    )
    assert result.returncode == 0, result.stderr

    # Verify the "modify bottom only" commit has ONLY the bottom change
    log = run_git(repo, "log", "--all", "--format=%H %s")
    for line in log.stdout.strip().split("\n"):
        if "modify bottom only" in line:
            sha = line.split()[0]
            show = run_git(repo, "show", sha)
            # The picked commit should have BOTTOM change, NOT TOP
            assert "BOTTOM MODIFIED" in show.stdout, \
                f"Expected BOTTOM MODIFIED in picked commit, got:\n{show.stdout}"
            assert "TOP MODIFIED" not in show.stdout, \
                f"TOP MODIFIED should NOT be in picked commit (wrong hunk staged)"
        elif "modify top only" in line:
            sha = line.split()[0]
            show = run_git(repo, "show", sha)
            # The rest commit should have TOP change, NOT BOTTOM
            assert "TOP MODIFIED" in show.stdout, \
                f"Expected TOP MODIFIED in rest commit"
            assert "BOTTOM MODIFIED" not in show.stdout, \
                f"BOTTOM MODIFIED should NOT be in rest commit"


def test_split_with_message_body(git_agent_exe, repo):
    """Test split with subject and body via multiple -m flags."""
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
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        "-m",
        "First change",
        "-m",
        "This is the body paragraph.",
        "--rest-message",
        "Remaining",
        "--rest-message",
        "Rest body paragraph.",
    )
    assert result.returncode == 0, result.stderr

    # Verify first commit has subject and body
    log = run_git(repo, "log", "--format=%B", "-n", "1", "HEAD~1")
    assert "First change" in log.stdout
    assert "This is the body paragraph." in log.stdout
    # Verify blank line separates subject and body
    assert "First change\n\nThis is the body" in log.stdout

    # Verify rest commit has subject and body
    log = run_git(repo, "log", "--format=%B", "-n", "1", "HEAD")
    assert "Remaining" in log.stdout
    assert "Rest body paragraph." in log.stdout
    assert "Remaining\n\nRest body" in log.stdout


def test_split_multi_hunks_multi_picks_with_line_ranges(git_agent_exe, repo):
    """Split multiple hunks from different files across multiple --pick groups.

    This tests that line ranges remain stable across pick groups by using
    stateful tracking of original hunks rather than re-reading the diff.
    """
    # Create initial files
    create_file(
        repo,
        "config.py",
        'DATABASE = "sqlite:///app.db"\n'
        'SECRET = "dev"\n'
        'DEBUG = True\n'
        'LOG_LEVEL = "INFO"\n',
    )
    create_file(
        repo,
        "server.py",
        "from flask import Flask, jsonify\n"
        "app = Flask(__name__)\n"
        "users = [\n"
        '    {"id": 1, "name": "Alice"},\n'
        '    {"id": 2, "name": "Bob"},\n'
        "]\n"
        "\n"
        "@app.route('/users')\n"
        "def list_users():\n"
        "    return jsonify(users)\n"
        "\n"
        "@app.route('/health')\n"
        "def health():\n"
        '    return jsonify({"status": "ok"})\n',
    )

    # Modify both files: add logging config + logging code, add pagination
    modify_file(
        repo,
        "config.py",
        'DATABASE = "sqlite:///app.db"\n'
        'SECRET = "dev"\n'
        'DEBUG = True\n'
        'LOG_LEVEL = "DEBUG"\n'  # logging concern
        'LOG_FORMAT = "%(asctime)s"\n'  # logging concern
        'MAX_PAGE_SIZE = 100\n',  # pagination concern
    )
    modify_file(
        repo,
        "server.py",
        "from flask import Flask, jsonify, request\n"
        "import logging\n"  # logging concern
        "app = Flask(__name__)\n"
        "logger = logging.getLogger(__name__)\n"  # logging concern
        "users = [\n"
        '    {"id": 1, "name": "Alice"},\n'
        '    {"id": 2, "name": "Bob"},\n'
        "]\n"
        "\n"
        "@app.route('/users')\n"
        "def list_users():\n"
        '    page = request.args.get("page", 1, type=int)\n'  # pagination
        "    return jsonify(users[page:page+10])\n"  # pagination
        "\n"
        "@app.route('/health')\n"
        "def health():\n"
        '    logger.info("health check")\n'  # logging concern
        '    return jsonify({"status": "ok"})\n',
    )
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-m", "add logging and pagination")

    # Get hunk IDs and show full output for debugging
    result = run_git_agent(git_agent_exe, repo, "hunks", "--commit", "HEAD", "--full")
    assert result.returncode == 0

    # Identify hunks by file
    config_id = None
    server_ids = []
    for line in result.stdout.strip().split("\n"):
        if line and not line.startswith(" ") and ":" not in line[:3]:
            parts = line.split()
            if len(parts) >= 2:
                hunk_id = parts[0]
                if "config.py" in line:
                    config_id = hunk_id
                elif "server.py" in line:
                    server_ids.append(hunk_id)
    assert config_id, f"Could not find config.py hunk: {result.stdout}"
    assert len(server_ids) >= 1, f"Could not find server.py hunks: {result.stdout}"

    # For simplicity, pick whole hunks for first group, then line ranges for second
    # This tests the core issue: after first pick commits, second pick fails
    # because the original hunk IDs are no longer valid
    result = run_git_agent(
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        f"{config_id}:4-6",  # logging config: LOG_LEVEL and LOG_FORMAT
        server_ids[0],  # first server.py hunk (imports/logger)
        "--message",
        "add logging",
        "--pick",
        f"{config_id}:7",  # MAX_PAGE_SIZE
        "--message",
        "add pagination config",
        "--rest-message",
        "add pagination code",
    )
    assert result.returncode == 0, f"split failed: {result.stderr}"

    subjects = _commit_subjects(repo)
    assert "add logging" in subjects
    assert "add pagination config" in subjects


def test_split_pick_after_rest_message_fails(git_agent_exe, repo):
    """Error when --pick appears after --rest-message."""
    content = "top\n" + "ctx\n" * 20 + "bottom\n"
    create_file(repo, "f.txt", content)

    new_content = "top modified\n" + "ctx\n" * 20 + "bottom modified\n"
    modify_file(repo, "f.txt", new_content)
    run_git(repo, "add", "f.txt")
    run_git(repo, "commit", "-m", "modify both")

    ids = _get_hunk_ids(git_agent_exe, repo, "--commit", "HEAD")

    result = run_git_agent(
        git_agent_exe,
        repo,
        "split",
        "HEAD",
        "--pick",
        ids[0],
        "-m",
        "first",
        "--rest-message",
        "rest",
        "--pick",
        ids[1],
        "-m",
        "should fail",
    )
    assert result.returncode != 0
    assert "not allowed after --rest-message" in result.stderr
