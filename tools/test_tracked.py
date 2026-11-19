"""A source file git ignores is a file the branch lacks."""
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUILD = ("target/", "tools/.venv/", "tools/.pytest_cache/", "__pycache__")


def test_no_source_file_is_hidden_from_git():
    """A bare `target` in `.git/info/exclude` ignored `crates/target/`: arm64's `-g` module and its fixtures were never committed, and #5911 did not build from a clean checkout."""
    hidden = subprocess.run(
        ["git", "ls-files", "--others", "--ignored", "--exclude-standard", "crates", "tools", "tests", "runtime"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout.split()
    hidden = [path for path in hidden if not path.startswith(BUILD) and "__pycache__" not in path]
    assert not hidden, hidden
