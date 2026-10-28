#!/usr/bin/env python3
"""Whether a pull request's merge ref carries main's ci.yml: a pull_request run reads its workflow from refs/pull/N/merge, not from main.

    merge_ref_fresh.py PR [REMOTE=origin]      exit 0 if fresh, 1 with the reason

#2340 was labelled merge-candidate and no run came: its merge ref was made hours before #2358 changed the workflow's `on:` (types [labeled]), and GitHub
does not remake the ref when the base moves or the label is added, so the old trigger (opened, synchronize, ...) still applied and a label matched nothing.
The loop calls this before it labels; a stale ref is remade by pushing main into the PR's branch (the candidate is the head with main merged anyway).
"""
from __future__ import annotations

import subprocess
import sys

WORKFLOW = ".github/workflows/ci.yml"


def on_block(text: str) -> str:
    """The `on:` block of a workflow file: from `on:` to the next top-level key."""
    lines, out, inside = text.splitlines(), [], False
    for line in lines:
        if line.startswith("on:"):
            inside = True
        elif inside and line and not line[0].isspace() and not line.startswith("#"):
            break
        if inside:
            out.append(line)
    return "\n".join(out)


def check(merge: str, main: str) -> tuple[bool, str]:
    """(fresh, why): the merge ref's workflow is main's, or at least runs the same triggers."""
    if merge == main:
        return True, "the merge ref carries main's ci.yml"
    if on_block(merge) != on_block(main):
        return False, "the merge ref's `on:` is not main's (its triggers are old: a label starts no run); push main into the branch to remake the ref"
    return False, "the merge ref's ci.yml differs from main's (its jobs are old); push main into the branch to remake the ref"


def git_show(ref: str) -> str:
    done = subprocess.run(["git", "show", f"{ref}:{WORKFLOW}"], capture_output=True, text=True)
    if done.returncode:
        raise SystemExit(f"cannot read {WORKFLOW} at {ref}: {done.stderr.strip()}")
    return done.stdout


def main() -> int:
    pr, remote = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else "origin")
    subprocess.run(["git", "fetch", "-q", remote, "main", f"refs/pull/{pr}/merge:refs/tmp/merge{pr}"], check=True)
    ok, why = check(git_show(f"refs/tmp/merge{pr}"), git_show(f"{remote}/main"))
    print(why)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
