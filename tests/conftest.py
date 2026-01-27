import subprocess
import pytest
from pathlib import Path
from dataclasses import dataclass


@dataclass
class CommandResult:
    returncode: int
    stdout: str
    stderr: str


@pytest.fixture(scope="session")
def git_agent_exe():
    root = Path(__file__).parent.parent
    subprocess.run(["cargo", "build"], cwd=root, check=True)
    return root / "target" / "debug" / "git-surgeon"


@pytest.fixture
def repo(tmp_path):
    subprocess.run(["git", "init"], cwd=tmp_path, check=True)
    subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=tmp_path, check=True)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=tmp_path, check=True)
    # Initial commit so HEAD exists
    (tmp_path / ".gitkeep").touch()
    subprocess.run(["git", "add", "."], cwd=tmp_path, check=True)
    subprocess.run(["git", "commit", "-m", "init"], cwd=tmp_path, check=True)
    return tmp_path


def run_git_agent(exe, repo, *args):
    result = subprocess.run(
        [str(exe), *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    return CommandResult(result.returncode, result.stdout, result.stderr)


def run_git(repo, *args):
    result = subprocess.run(
        ["git", *args],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    return CommandResult(result.returncode, result.stdout, result.stderr)


def create_file(repo, path, content):
    """Write a file, git add, and commit it."""
    filepath = repo / path
    filepath.parent.mkdir(parents=True, exist_ok=True)
    filepath.write_text(content)
    subprocess.run(["git", "add", str(path)], cwd=repo, check=True)
    subprocess.run(["git", "commit", "-m", f"add {path}"], cwd=repo, check=True)


def modify_file(repo, path, content):
    """Overwrite file content without committing."""
    filepath = repo / path
    filepath.write_text(content)
