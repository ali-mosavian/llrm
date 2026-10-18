#!/usr/bin/env python3
"""The one way to squash a pull request into feat/parity: a bare `gh pr merge` merged stale heads that reverted other PRs' work (#2329 reverted decide's LVI step).

    tools/parity_squash.py merge PR [--known-red STEP=ISSUE]...   refuse, or run the fast set on this checkout's head (it must be the PR's) and `gh pr merge --squash`
    tools/parity_squash.py audit [SINCE] every squash on origin/feat/parity since SINCE (default 96468410e): what it deleted outside its own PR's files

Refused unless: the PR head contains the current origin/feat/parity; the squash diff deletes no test function (`#[test] fn`, `def test_`) and no more than
LIMIT lines in files its own non-merge commits did not touch, or the description has a `Reverts:` line; parity_smoke passes on that exact head.
`--known-red sc=2438` skips ONLY that smoke step (named, with its issue), still runs every other check, and puts one line in the squash's body:
`Known red: sc (#2438)`.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIMIT = 20  # deleted lines allowed in files the PR's own commits did not touch
BASE = "origin/feat/parity"
TEST_RUST = re.compile(r"^-\s*#\[test\]")
TEST_PY = re.compile(r"^-\s*def (test_\w+)")


def git(*args: str, cwd: Path = ROOT) -> str:
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if done.returncode:
        raise SystemExit(f"git {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout


def deleted_tests(diff: str) -> int:
    """Test functions a unified diff removes (a removed `#[test]` or `def test_`)."""
    return sum(1 for line in diff.splitlines() if TEST_RUST.match(line) or TEST_PY.match(line))


def outside_files(base: str, head: str, cwd: Path = ROOT) -> list[str]:
    """Files the squash (base..head net diff) changes that the PR's own non-merge commits did not touch."""
    net = set(git("diff", "--name-only", base, head, cwd=cwd).split())
    own = set(git("log", "--no-merges", "--format=", "--name-only", f"{base}..{head}", cwd=cwd).split())
    return sorted(net - own)


def refusals(head: str, description: str, base: str = BASE, cwd: Path = ROOT) -> list[str]:
    """Why this head may not be squashed into `base`; empty if it may."""
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=cwd).returncode:
        return [f"the PR head does not contain {base}: merge it in first (a stale head squashes other PRs' work away)"]
    out = outside_files(base, head, cwd)
    why = []
    allowed = bool(re.search(r"^Reverts:", description, re.M))
    for path in out:
        diff = git("diff", base, head, "--", path, cwd=cwd)
        gone = sum(1 for line in diff.splitlines() if line.startswith("-") and not line.startswith("---"))
        tests = deleted_tests(diff)
        if (tests or gone > LIMIT) and not allowed:
            why.append(f"{path}: the squash deletes {tests} test function(s) and {gone} line(s) in a file the PR's own commits did not touch (a `Reverts:` line in the description allows it)")
    return why


def audit(since: str, cwd: Path = ROOT) -> list[tuple[str, str, str]]:
    """(sha, subject, finding) for each first-parent squash since `since` that deleted tests or more than LIMIT lines in files its PR's own non-merge commits did not touch.
    The PR's files are read from its head's commits, not from GitHub's file list: that list is a diff against main, so a stale head's reverts are in it."""
    squashes = []
    for sha in git("log", "--first-parent", "--format=%H", f"{since}..{BASE}", cwd=cwd).split():
        subject = git("log", "-1", "--format=%s", sha, cwd=cwd).strip()
        m = re.search(r"\(#?(\d+)\)$", subject)
        squashes.append((sha, subject, m[1] if m else None))
    numbers = sorted({n for _, _, n in squashes if n})
    subprocess.run(["git", "fetch", "-q", "origin", *[f"+refs/pull/{n}/head:refs/prh/{n}" for n in numbers]], cwd=cwd, capture_output=True)
    rows = []
    for sha, subject, number in squashes:
        files = set(git("diff", "--name-only", f"{sha}^", sha, cwd=cwd).split())
        head = subprocess.run(["git", "rev-parse", "-q", "--verify", f"refs/prh/{number}"], cwd=cwd, capture_output=True, text=True).stdout.strip() if number else ""
        # A direct landing (no PR number, or a PR head GitHub no longer serves) has no own-commit set: only deleted tests are reported for it.
        mine = set(git("log", "--no-merges", "--format=", "--name-only", f"{sha}^..{head}", cwd=cwd).split()) if head else None
        for path in sorted(files if mine is None else files - mine):
            diff = git("diff", f"{sha}^", sha, "--", path, cwd=cwd)
            gone = sum(1 for line in diff.splitlines() if line.startswith("-") and not line.startswith("---"))
            tests = deleted_tests(diff)
            if tests or (gone > LIMIT and mine is not None):
                rows.append((sha[:9], subject, f"{path}: -{gone} lines, {tests} test fn deleted" + ("" if mine is not None else " (direct landing: any file)")))
    return rows


SMOKE_STEPS = ("programs", "sc", "d_faces", "run")


def parse_known_red(args: list[str]) -> dict[str, str]:
    """`STEP=ISSUE` pairs -> {step: '#ISSUE'}; a step parity_smoke does not have, or no issue, is refused."""
    out = {}
    for arg in args:
        step, _, issue = arg.partition("=")
        if step not in SMOKE_STEPS or not issue.lstrip("#").isdigit():
            raise SystemExit(f"--known-red {arg!r}: want STEP=ISSUE with STEP one of {', '.join(SMOKE_STEPS)} and ISSUE a number")
        out[step] = "#" + issue.lstrip("#")
    return out


def known_red_line(known: dict[str, str]) -> str:
    return "Known red: " + ", ".join(f"{step} ({issue})" for step, issue in known.items()) if known else ""


def merge(number: str, known: dict[str, str] | None = None) -> int:
    known = known or {}
    info = json.loads(subprocess.run(["gh", "pr", "view", number, "--json", "headRefOid,title,body,baseRefName"], capture_output=True, text=True, check=True).stdout)
    if info["baseRefName"] != "feat/parity":
        raise SystemExit(f"#{number} targets {info['baseRefName']}, not feat/parity")
    git("fetch", "-q", "origin", "feat/parity")
    here = git("rev-parse", "HEAD").strip()
    if here != info["headRefOid"]:
        raise SystemExit(f"this checkout is {here[:9]}, the PR head is {info['headRefOid'][:9]}: check the PR head out first (the fast set runs on exactly it)")
    if why := refusals(info["headRefOid"], info["body"] or ""):
        print("\n".join(f"REFUSED: {w}" for w in why))
        return 1
    bins = os.environ.get("PARITY_BIN")
    if not bins:
        raise SystemExit("PARITY_BIN names the directory of the built release llrm-c, llrm-qb, llrm-nib (parity_smoke needs them)")
    smoke = subprocess.run(["uv", "run", "-q", "--project", "tools", "python", "tools/parity_smoke.py", "--bin", bins, "--base", BASE, *[a for step in known for a in ("--skip", step)]], cwd=ROOT, capture_output=True, text=True)
    last = (smoke.stdout.strip().splitlines() or [""])[-1]
    print(last)
    if smoke.returncode:
        print("REFUSED: the fast set fails on this head")
        return 1
    return subprocess.run(["gh", "pr", "merge", number, "--squash", "--subject", info["title"], "--body", known_red_line(known)]).returncode


def main() -> int:
    if len(sys.argv) >= 3 and sys.argv[1] == "merge":
        rest = sys.argv[3:]
        return merge(sys.argv[2], parse_known_red([rest[i + 1] for i, a in enumerate(rest) if a == "--known-red" and i + 1 < len(rest)]))
    if len(sys.argv) >= 2 and sys.argv[1] == "audit":
        since = sys.argv[2] if len(sys.argv) > 2 else "96468410e"
        rows = audit(since)
        for sha, subject, finding in rows:
            print(f"{sha}  {subject[:70]}  {finding}")
        print(f"{len(rows)} finding(s)")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
