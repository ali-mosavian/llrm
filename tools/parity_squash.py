#!/usr/bin/env python3
"""The one way to squash a pull request into feat/parity: a bare `gh pr merge` merged stale heads that reverted other PRs' work (#2329 reverted decide's LVI step).

    [PARITY_BIN=DIR] tools/parity_squash.py merge PR [--known-red STEP=ISSUE]...   under /tmp/land.lock (land.sh's): merge feat/parity into the PR head here (not pushed), refuse or run the fast set on THAT, then `gh pr merge --squash --match-head-commit`
    tools/parity_squash.py audit [SINCE] every squash on origin/feat/parity since SINCE (default 96468410e): what it deleted without a Deletes:/Reverts: line in its PR's description

Refused unless: the PR head contains the current origin/feat/parity; the squash diff deletes no file, no test function (`#[test] fn`, `def test_`, unless re-added) and no more than
LIMIT lines of a file, unless a `Deletes:` or `Reverts:` line of the description names the path (own commits or not: #2443's deleted scev/expand_iv.rs); parity_smoke passes on that exact head.
PARITY_BIN names the directory of the built release llrm-c, llrm-qb and llrm-nib (parity_smoke needs them); unset, it is the repo target dir's release
(LLRM_BIN / CARGO_TARGET_DIR as tools/llrmbin.py reads them) when all three are there.
`--known-red sc=2438` skips ONLY that smoke step (named, with its issue), still runs every other check, and puts one line in the squash's body:
`Known red: sc (#2438)`.
"""
from __future__ import annotations

import contextlib
import fcntl
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIMIT = 20  # deleted lines allowed in files the PR's own commits did not touch
BASE = "origin/feat/parity"
LAND_LOCK = "/tmp/land.lock"  # the coordinator's land.sh holds the same file: one landing at a time, whoever lands
TEST_RUST = re.compile(r"^-\s*#\[test\]")
TEST_PY = re.compile(r"^-\s*def (test_\w+)")


def git(*args: str, cwd: Path = ROOT) -> str:
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if done.returncode:
        raise SystemExit(f"git {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout


FN = re.compile(r"^[-+]\s*(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+(\w+)|^[-+]\s*def\s+(test_\w+)")


def removed_tests(diff: str) -> set[str]:
    """Names of the test functions a unified diff removes and does not add back (a moved test is not a deleted one)."""
    gone, added, pending = set(), set(), False
    for line in diff.splitlines():
        if line.startswith("-") and not line.startswith("---") and re.match(r"^-\s*#\[test\]", line):
            pending = True
            continue
        found = FN.match(line)
        name = (found[1] or found[2]) if found else None
        if line.startswith("+") and name:
            added.add(name)
        elif line.startswith("-") and name and (pending or found[2]):
            gone.add(name)
        if not line.startswith("-") or found:
            pending = False
    return gone - added


def deleted_tests(diff: str) -> int:
    return len(removed_tests(diff))


def outside_files(base: str, head: str, cwd: Path = ROOT) -> list[str]:
    """Files the squash (base..head net diff) changes that the PR's own non-merge commits did not touch."""
    net = set(git("diff", "--name-only", base, head, cwd=cwd).split())
    own = set(git("log", "--no-merges", "--format=", "--name-only", f"{base}..{head}", cwd=cwd).split())
    return sorted(net - own)


def losses(base: str, head: str, cwd: Path = ROOT) -> list[tuple[str, int, int, bool]]:
    """(path, deleted lines, test functions removed, whole file gone) for each file of the net diff base..head that loses more than LIMIT lines, loses a test or goes.
    Own commits or not: #2443's own commit deleted scev/expand_iv.rs (855 lines) from a stale tree, and 'outside its own commits' let it through."""
    out = []
    for line in git("diff", "--numstat", base, head, cwd=cwd).splitlines():
        added, gone, path = line.split("\t", 2)
        if gone == "-":
            continue
        diff = git("diff", base, head, "--", path, cwd=cwd)
        tests = len(removed_tests(diff))
        whole = bool(re.search(r"^deleted file mode", diff, re.M))
        if int(gone) > LIMIT or tests or whole:
            out.append((path, int(gone), tests, whole))
    return out


def acknowledged(description: str, path: str) -> bool:
    """Whether a `Reverts:` or `Deletes:` line of the description names `path` (or its file name): the author knows what the squash removes."""
    names = {path, Path(path).name}
    return any(any(n in line for n in names) for line in description.splitlines() if re.match(r"^(Reverts|Deletes):", line))


def refusals(head: str, description: str, base: str = BASE, cwd: Path = ROOT) -> list[str]:
    """Why this head may not be squashed into `base`; empty if it may."""
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=cwd).returncode:
        return [f"the PR head does not contain {base}: merge it in first (a stale head squashes other PRs' work away)"]
    outside = set(outside_files(base, head, cwd))
    why = []
    for path, gone, tests, whole in losses(base, head, cwd):
        if not acknowledged(description, path):
            what = "deletes the whole file" if whole else f"deletes {gone} line(s)"
            why.append(f"{path}: the squash {what}{f' and {tests} test function(s)' if tests else ''}{' (no commit of this PR touched it)' if path in outside else ''}; "
                       "name the path in a `Deletes:` or `Reverts:` line of the description if that is meant")
    return why


def audit(since: str, cwd: Path = ROOT) -> list[tuple[str, str, str]]:
    """(sha, subject, finding) for each first-parent squash since `since` that deletes a file, a test or more than LIMIT lines of a file without a `Deletes:`/`Reverts:` line
    naming it in its PR's description. Own commits count: the guard is on the squash's net diff, not on who touched the file."""
    rows = []
    for sha in git("log", "--first-parent", "--format=%H", f"{since}..{BASE}", cwd=cwd).split():
        subject = git("log", "-1", "--format=%s", sha, cwd=cwd).strip()
        m = re.search(r"\(#?(\d+)\)$", subject)
        found = losses(f"{sha}^", sha, cwd)
        if not found:
            continue
        body = pr_body(m[1]) if m else ""
        for path, gone, tests, whole in found:
            if not acknowledged(body, path):
                rows.append((sha[:9], subject, f"{path}: -{gone} lines" + (", file deleted" if whole else "") + (f", {tests} test fn removed" if tests else "") + ("" if m else " (direct landing)")))
    return rows


def pr_body(number: str) -> str:
    done = subprocess.run(["gh", "pr", "view", number, "--json", "body", "--jq", ".body"], capture_output=True, text=True)
    return done.stdout if done.returncode == 0 else ""


BINARIES = ("llrm-c", "llrm-qb", "llrm-nib")


def smoke_bins(env: dict | None = None) -> str:
    """PARITY_BIN, else the release dir llrmbin finds when the three binaries are in it; else SystemExit saying what to build."""
    env = os.environ if env is None else env
    if env.get("PARITY_BIN"):
        return env["PARITY_BIN"]
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import llrmbin  # noqa: PLC0415

    found = llrmbin.bin_dir(env)
    if all((found / name).is_file() for name in BINARIES):
        return str(found)
    raise SystemExit(f"PARITY_BIN is unset and {found} lacks {', '.join(n for n in BINARIES if not (found / n).is_file())}: `cargo build --release --bin llrm-c --bin llrm-qb --bin llrm-nib`, or set PARITY_BIN to a directory that has them")


SMOKE_STEPS = ("programs", "sc", "d_faces", "qc_d_faces", "qc_modules", "run")


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


def stale_at_the_call(head: str, fetch=None, base: str = BASE, cwd: Path = ROOT) -> list[str]:
    """The containment check read at the instant of the merge call: feat/parity moves while the fast set runs (#2443's head predated #2429 by then)."""
    (fetch or (lambda: git("fetch", "-q", "origin", "feat/parity", cwd=cwd)))()
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=cwd).returncode:
        return [f"feat/parity moved while the checks ran: the PR head does not contain {base} now; merge it in and run again"]
    return []


def squash_strays(squash: str, expected: set[str], cwd: Path = ROOT) -> list[tuple[str, str]]:
    """(path, change) for each file of the landed squash (`squash^..squash`) outside the PR's own file set `expected` (gh's file list, taken before the merge):
    a revert by a stale tree shows as files the PR never listed."""
    out = []
    for line in git("diff", "--numstat", f"{squash}^", squash, cwd=cwd).splitlines():
        added, gone, path = line.split("\t", 2)
        if path not in expected:
            out.append((path, f"-{gone}/+{added}"))
    return out


@contextlib.contextmanager
def land_lock(path: str = LAND_LOCK, wait: bool = True):
    """The exclusive flock every landing holds, this script's and land.sh's alike. `wait=False` raises BlockingIOError when another landing holds it."""
    with open(path, "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | (0 if wait else fcntl.LOCK_NB))
        try:
            yield
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


def merged_result(head: str, base: str = BASE, cwd: Path = ROOT) -> tuple[str | None, str]:
    """(sha, "") of `head` with `base` merged into it, checked out detached in `cwd` and not pushed; (None, why) when `base` conflicts. When `head` already
    contains `base` the result is `head` itself. The squash lands on whatever feat/parity is at the call: the tests must run on this result, not on a head that
    compiled beside a base that has since moved (#2544 and #2564 each compiled; together the head did not build for three hours)."""
    git("checkout", "-q", "--detach", head, cwd=cwd)
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=cwd).returncode:
        merged = subprocess.run(["git", "merge", "--no-edit", "-q", base], cwd=cwd, capture_output=True, text=True)
        if merged.returncode:
            conflicts = git("diff", "--name-only", "--diff-filter=U", cwd=cwd).split()
            subprocess.run(["git", "merge", "--abort"], cwd=cwd, capture_output=True)
            return None, f"merging {base} into the PR head conflicts in {', '.join(conflicts) or 'the tree'}"
    return git("rev-parse", "HEAD", cwd=cwd).strip(), ""


def merged_refusals(head: str, base: str = BASE, check=None, cwd: Path = ROOT) -> list[str]:
    """Why the PR at `head` may not land on `base` as it stands now: the merge conflicts, or the merged tree does not compile as a whole (`check(cwd)` is
    parity_smoke.workspace_check: "" when it does)."""
    sha, why = merged_result(head, base, cwd)
    if sha is None:
        return [why]
    broken = (check or _workspace_check)(cwd)
    return [f"the PR head merged with {base} does not build as a whole: {broken}"] if broken else []


def _workspace_check(cwd: Path) -> str:
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import parity_smoke

    return parity_smoke.workspace_check(cwd)


def merge(number: str, known: dict[str, str] | None = None) -> int:
    with land_lock():
        return _merge(number, known)


def _merge(number: str, known: dict[str, str] | None = None) -> int:
    known = known or {}
    info = json.loads(subprocess.run(["gh", "pr", "view", number, "--json", "headRefOid,title,body,baseRefName"], capture_output=True, text=True, check=True).stdout)
    if info["baseRefName"] != "feat/parity":
        raise SystemExit(f"#{number} targets {info['baseRefName']}, not feat/parity")
    git("fetch", "-q", "origin", "feat/parity")
    if git("status", "--porcelain", "--untracked-files=no").strip():
        raise SystemExit("this checkout has uncommitted changes: the fast set runs on the PR head merged with feat/parity, checked out here")
    git("fetch", "-q", "origin", info["headRefOid"])
    tested_base = git("rev-parse", BASE).strip()
    merged, why = merged_result(info["headRefOid"])
    if merged is None:
        print(f"REFUSED: {why}")
        return 1
    if why := refusals(merged, info["body"] or ""):
        print("\n".join(f"REFUSED: {w}" for w in why))
        return 1
    bins = smoke_bins()
    smoke = subprocess.run(["uv", "run", "-q", "--project", "tools", "python", "tools/parity_smoke.py", "--bin", bins, "--base", BASE, *[a for step in known for a in ("--skip", step)]], cwd=ROOT, capture_output=True, text=True)
    last = (smoke.stdout.strip().splitlines() or [""])[-1]
    print(last)
    if smoke.returncode:
        print("REFUSED: the fast set fails on this head")
        return 1
    files = {f["path"] for f in json.loads(subprocess.run(["gh", "pr", "view", number, "--json", "files"], capture_output=True, text=True, check=True).stdout)["files"]}
    if why := stale_at_the_call(merged):
        print("\n".join(f"REFUSED: {w}" for w in why))
        return 1
    if git("rev-parse", BASE).strip() != tested_base:
        print(f"REFUSED: feat/parity moved from {tested_base[:9]} to {git('rev-parse', BASE).strip()[:9]} while the checks ran; run again")
        return 1
    done = subprocess.run(["gh", "pr", "merge", number, "--squash", "--match-head-commit", info["headRefOid"], "--subject", info["title"], "--body", known_red_line(known)])
    if done.returncode:
        return done.returncode
    git("fetch", "-q", "origin", "feat/parity")
    squash = json.loads(subprocess.run(["gh", "pr", "view", number, "--json", "mergeCommit"], capture_output=True, text=True, check=True).stdout)["mergeCommit"]["oid"]
    git("fetch", "-q", "origin", squash)
    if strays := squash_strays(squash, files):
        print(f"REVERT-DETECTED: squash {squash[:9]} of #{number} touches files the PR never listed (a stale tree reverting other work?); restore them NOW:")
        print("\n".join(f"  {path} {change}" for path, change in strays))
        return 3
    return 0


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
