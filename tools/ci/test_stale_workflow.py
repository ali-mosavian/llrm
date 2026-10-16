import sys
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import stale_workflow  # noqa: E402


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t@t", *args], capture_output=True, text=True, check=True).stdout.strip()


def commit(root: Path, path: str, text: str) -> str:
    (root / path).parent.mkdir(parents=True, exist_ok=True)
    (root / path).write_text(text)
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", path)
    return git(root, "rev-parse", "HEAD")


def repo(tmp_path: Path) -> Path:
    git(tmp_path, "init", "-q", "-b", "main")
    commit(tmp_path, "a.txt", "1")
    return tmp_path


def merge_commit(root: Path, parent: str) -> None:
    """A pull request's merge commit with `parent` as its first parent, checked out."""
    git(root, "checkout", "-q", "-b", "pr", parent)
    commit(root, "pr.txt", "x")
    git(root, "checkout", "-q", "--detach", parent)
    git(root, "merge", "-q", "--no-ff", "-m", "merge", "pr")


def test_a_run_on_a_parent_older_than_the_last_workflow_change_is_stale(tmp_path):
    """#1866 passed on the workflow from before #1890 and #1898: its merge parent lacked both."""
    root = repo(tmp_path)
    old = git(root, "rev-parse", "HEAD")
    commit(root, ".github/workflows/ci.yml", "new")
    git(root, "update-ref", "refs/remotes/origin/main", "main")
    merge_commit(root, old)
    assert stale_workflow.verdict(root) == (old[:12], "stale")


def test_a_run_on_a_parent_with_the_last_workflow_change_is_fresh_though_main_moved_on(tmp_path):
    root = repo(tmp_path)
    parent = commit(root, "tools/gate/gate.py", "new")
    commit(root, "docs/x.md", "later, not a workflow file")
    git(root, "update-ref", "refs/remotes/origin/main", "main")
    merge_commit(root, parent)
    assert stale_workflow.verdict(root) == (parent[:12], "fresh")


def test_a_pull_request_whose_base_is_not_main_is_stacked(tmp_path):
    root = repo(tmp_path)
    git(root, "update-ref", "refs/remotes/origin/main", "main")
    merge_commit(root, git(root, "rev-parse", "HEAD"))
    assert stale_workflow.verdict(root, base_ref="feature")[1] == "stacked"
