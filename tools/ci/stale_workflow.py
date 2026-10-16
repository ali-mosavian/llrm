"""Whether a pull request's run used an older workflow than main's.

    stale_workflow.py [ROOT]    prints "PARENT12 stale|fresh|stacked" for the checked-out merge commit of a pull request

A pull request runs the workflow and the gate of its merge commit, whose first parent is the base it was last merged with. When main has
bumped the semantics marker (`tools/ci/workflow-version`) since, a run on an older parent judges by the old rules (#1866 passed on the workflow
from before #1890 and #1898). A change that only makes the run cheaper (fewer jobs, other runners) does not bump it: it would send every parked pull
request back through a full gate. `stacked` is a base that is not main: it runs that branch's workflow.
"""

import sys
import subprocess
from pathlib import Path

MARKER = "tools/ci/workflow-version"  # an integer, bumped by a commit that changes what a run checks or how it judges (not by one that only makes it cheaper)


def git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True, check=check)


def verdict(root: Path, parent: str = "HEAD^1", main: str = "origin/main", base_ref: str = "main") -> tuple[str, str]:
    """(the parent's first 12 characters, "fresh", "stale" or "stacked")."""
    sha = git(root, "rev-parse", parent).stdout.strip()
    if base_ref != "main":
        return sha[:12], "stacked"
    last = git(root, "log", "-1", "--format=%H", main, "--", MARKER).stdout.strip()
    contains = not last or git(root, "merge-base", "--is-ancestor", last, sha, check=False).returncode == 0
    return sha[:12], "fresh" if contains else "stale"


if __name__ == "__main__":
    import os

    print(*verdict(Path(sys.argv[1] if len(sys.argv) > 1 else "."), base_ref=os.environ.get("BASE_REF", "main")))
