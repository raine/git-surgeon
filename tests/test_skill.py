import json
import subprocess
from pathlib import Path

import pytest


SKILL_SOURCE = Path(__file__).parent.parent / "skills" / "git-surgeon" / "SKILL.md"
LAYOUTS = {
    "claude": ".claude/skills/git-surgeon/SKILL.md",
    "pi": ".pi/agent/skills/git-surgeon/SKILL.md",
    "opencode": ".config/opencode/skills/git-surgeon/SKILL.md",
    "codex": ".codex/skills/git-surgeon/SKILL.md",
}


def run(exe, *args):
    return subprocess.run([str(exe), *args], capture_output=True, text=True)


def test_skill_list_json_and_version_catalog(git_agent_exe):
    listed = run(git_agent_exe, "skill", "list", "--json")
    assert listed.returncode == 0
    payload = json.loads(listed.stdout)
    assert payload["schema_version"] == 1
    assert len(payload["skills"]) == 1
    skill = payload["skills"][0]
    assert skill == {
        "name": "git-surgeon",
        "description": "Non-interactive hunk-level git staging, unstaging, discarding, undoing, fold, amend, squash, commit splitting, and commit reordering.",
        "cli_version": "0.1.17",
        "schema_version": 1,
        "path_in_repo": "skills/git-surgeon/SKILL.md",
    }

    version = run(git_agent_exe, "version", "--json")
    assert version.returncode == 0
    version_payload = json.loads(version.stdout)
    assert version_payload["version"] == "0.1.17"
    assert version_payload["schema_version"] == 1
    assert version_payload["supported_schemas"] == [1]
    assert version_payload["skills"] == [skill]
    assert len(version_payload["commit"]) == 40
    int(version_payload["commit"], 16)


@pytest.mark.parametrize("verb", ["print", "show"])
def test_skill_print_and_show_are_byte_faithful(git_agent_exe, verb):
    result = subprocess.run(
        [str(git_agent_exe), "skill", verb, "git-surgeon"], capture_output=True
    )
    assert result.returncode == 0
    assert result.stdout == SKILL_SOURCE.read_bytes()

    structured = run(git_agent_exe, "skill", verb, "git-surgeon", "--json")
    payload = json.loads(structured.stdout)
    assert payload["content"].encode() == SKILL_SOURCE.read_bytes()
    assert payload["cli_version"] == "0.1.17"
    assert payload["skill_schema_version"] == 1


@pytest.mark.parametrize("target,relative", LAYOUTS.items())
def test_skill_install_exact_runtime_layout(git_agent_exe, tmp_path, target, relative):
    result = run(
        git_agent_exe,
        "skill",
        "install",
        "git-surgeon",
        "--target",
        target,
        "--target-root",
        str(tmp_path),
    )
    assert result.returncode == 0, result.stderr
    installed = tmp_path / relative
    assert installed.read_bytes() == SKILL_SOURCE.read_bytes()
    assert [path for path in tmp_path.rglob("SKILL.md")] == [installed]


@pytest.mark.parametrize("target,relative", LAYOUTS.items())
def test_legacy_install_skill_layouts(git_agent_exe, tmp_path, target, relative):
    result = run(
        git_agent_exe,
        "install-skill",
        f"--{target}",
        "--target-root",
        str(tmp_path),
    )
    assert result.returncode == 0, result.stderr
    assert (tmp_path / relative).read_bytes() == SKILL_SOURCE.read_bytes()


def test_install_all_and_idempotency_json(git_agent_exe, tmp_path):
    args = (
        "skill",
        "install",
        "--target",
        "all",
        "--target-root",
        str(tmp_path),
        "--json",
    )
    first = run(git_agent_exe, *args)
    assert [item["platform"] for item in json.loads(first.stdout)["results"]] == list(
        LAYOUTS
    )
    for relative in LAYOUTS.values():
        assert (tmp_path / relative).read_bytes() == SKILL_SOURCE.read_bytes()

    second = run(git_agent_exe, *args)
    assert {item["action"] for item in json.loads(second.stdout)["results"]} == {
        "unchanged"
    }


def test_dry_run_reports_without_writes(git_agent_exe, tmp_path):
    result = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "all",
        "--target-root",
        str(tmp_path),
        "--dry-run",
        "--json",
    )
    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["dry_run"] is True
    assert {item["action"] for item in payload["results"]} == {"would_install"}
    assert not list(tmp_path.iterdir())


def test_managed_upgrade_and_clobber_protection(git_agent_exe, tmp_path):
    destination = tmp_path / LAYOUTS["pi"]
    destination.parent.mkdir(parents=True)

    destination.write_text("unmanaged\n")
    refused = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "pi",
        "--target-root",
        str(tmp_path),
    )
    assert refused.returncode != 0
    assert "unmanaged" in refused.stderr
    assert destination.read_text() == "unmanaged\n"

    forced = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "pi",
        "--target-root",
        str(tmp_path),
        "--force",
    )
    assert forced.returncode == 0
    assert destination.read_bytes() == SKILL_SOURCE.read_bytes()

    older = SKILL_SOURCE.read_text().replace("cli_version=0.1.17;", "cli_version=0.1.16;")
    destination.write_text(older)
    upgraded = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "pi",
        "--target-root",
        str(tmp_path),
    )
    assert upgraded.returncode == 0
    assert "upgraded" in upgraded.stdout
    assert destination.read_bytes() == SKILL_SOURCE.read_bytes()

    modified = SKILL_SOURCE.read_text().replace("# git-surgeon", "# locally edited")
    destination.write_text(modified)
    refused = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "pi",
        "--target-root",
        str(tmp_path),
    )
    assert refused.returncode != 0
    assert "modified managed skill" in refused.stderr
    assert destination.read_text() == modified

    newer = SKILL_SOURCE.read_text().replace("cli_version=0.1.17;", "cli_version=9.0.0;")
    destination.write_text(newer)
    refused = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "pi",
        "--target-root",
        str(tmp_path),
    )
    assert refused.returncode != 0
    assert "newer managed skill" in refused.stderr
    assert destination.read_text() == newer

    forced = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "pi",
        "--target-root",
        str(tmp_path),
        "--force",
    )
    assert forced.returncode == 0
    assert destination.read_bytes() == SKILL_SOURCE.read_bytes()


def test_install_all_validates_before_writing(git_agent_exe, tmp_path):
    conflict = tmp_path / LAYOUTS["codex"]
    conflict.parent.mkdir(parents=True)
    conflict.write_text("unmanaged\n")

    result = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "all",
        "--target-root",
        str(tmp_path),
    )
    assert result.returncode != 0
    assert list(tmp_path.rglob("SKILL.md")) == [conflict]


def test_unknown_skill_target_and_legacy_without_target_fail(git_agent_exe, tmp_path):
    unknown = run(git_agent_exe, "skill", "print", "other", "--json")
    assert unknown.returncode == 1
    unknown_error = json.loads(unknown.stderr)
    assert unknown_error["error"]["code"] == "command_error"
    assert "unknown skill 'other'" in unknown_error["error"]["message"]

    target = run(
        git_agent_exe,
        "skill",
        "install",
        "--target",
        "other",
        "--target-root",
        str(tmp_path),
        "--json",
    )
    assert target.returncode == 1
    target_error = json.loads(target.stderr)
    assert target_error["error"]["code"] == "invalid_arguments"
    assert "invalid value 'other'" in target_error["error"]["message"]

    legacy = run(git_agent_exe, "install-skill", "--target-root", str(tmp_path))
    assert legacy.returncode != 0
    assert "at least one platform flag" in legacy.stderr
