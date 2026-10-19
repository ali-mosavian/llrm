#!/usr/bin/env python3
"""The one way to squash a pull request into feat/parity: a bare `gh pr merge` merged stale heads that reverted other PRs' work (#2329 reverted decide's LVI step).

    [PARITY_BIN=DIR] tools/parity_squash.py merge PR [--known-red STEP=ISSUE]...   under /tmp/land.lock (land.sh's): merge feat/parity into the PR head here (not pushed), refuse or run the fast set on THAT, then `gh pr merge --squash --match-head-commit`
    tools/parity_squash.py merge-batch N1..N4 [--known-red STEP=ISSUE]...   up to four ready PRs: ONE fast set on the base with all merged in; on failure halve (the passing half lands first)
    tools/parity_squash.py audit [SINCE] every squash on origin/feat/parity since SINCE (default 96468410e): what it deleted without a Deletes:/Reverts: line in its PR's description

Subset rule: the fast set runs on the head (origin/feat/parity, failure set cached per sha by `head-failures`) AND on the merged tree; the merged tree may fail only checks the head fails, and strictly fewer
when the description has a `Fixes red` line; both sets are in the refusal. Every landing appends `<sha> #PR gate-ok <date>` to ~/scratch/coordwork/landed.log; `unlogged` lists feat/parity commits with no line.
Refused unless: the PR head contains the current origin/feat/parity; the squash diff deletes no file, no test function (`#[test] fn`, `def test_`, unless re-added) and no more than
LIMIT lines of a file, unless a `Deletes:` or `Reverts:` line of the description names the path (own commits or not: #2443's deleted scev/expand_iv.rs); parity_smoke passes on that exact head.
PARITY_BIN names the directory of the built release llrm-c, llrm-qb and llrm-nib (parity_smoke needs them); unset, it is the repo target dir's release
(LLRM_BIN / CARGO_TARGET_DIR as tools/llrmbin.py reads them) when all three are there.
`--known-red sc=2438` skips ONLY that smoke step (named, with its issue), still runs every other check, and puts one line in the squash's body:
`Known red: sc (#2438)`.
"""
from __future__ import annotations

import contextlib
import datetime
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
HEAD_FAILURES = Path.home() / "scratch/results/headfail"  # <sha>.json: the failing checks of the fast set on that head
LANDED_LOG = Path(os.environ.get("LANDED_LOG", Path.home() / "scratch/coordwork/landed.log"))
FIXES_RED = re.compile(r"(?im)^\s*fixes\s+(?:a\s+)?red\b")
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


def ensure_bins(env: dict | None = None, build=None, cwd: Path = ROOT) -> str:
    """"" when the binaries the smoke will use are newer than the tree's sources; else rebuild them (`buildslot cargo build --release --bins`, CARGO_TARGET_DIR from
    the environment) and check again, and say which are still stale. The merge rewrites files after the binaries were built: llrmbin.stale_binaries then flagged them,
    the smoke exited non-zero with an empty last line, and the tool printed no reason; nothing landed through the gate for a day."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import llrmbin  # noqa: PLC0415
    from buildslot import slotted  # noqa: PLC0415

    directory = Path(smoke_bins(env))
    if not llrmbin.stale_binaries(directory, cwd):
        return ""
    (build or (lambda: subprocess.run(slotted(["cargo", "build", "--release", "--bins", "-q"]), cwd=cwd, env={**os.environ, **(env or {})}, capture_output=True, text=True)))()
    still = llrmbin.stale_binaries(directory, cwd)
    return f"binaries still older than the merged tree after a rebuild: {', '.join(still)} in {directory}" if still else ""


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


def run_smoke(known: dict[str, str] | None = None) -> tuple[bool, str]:
    """(passed, the last line) of parity_smoke on this checkout; the known-red steps are skipped by name."""
    known = known or {}
    smoke = subprocess.run(
        ["uv", "run", "-q", "--project", "tools", "python", "tools/parity_smoke.py", "--bin", smoke_bins(), "--base", BASE, *[a for step in known for a in ("--skip", step)]],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return smoke.returncode == 0, (smoke.stdout.strip().splitlines() or [""])[-1]


def batch(prs: list, passes, land, refuse) -> None:
    """Land `prs` (up to four, ready, in order) with ONE test of the combined tree. `passes(prs)` merges them all into the base and runs the fast set; when it fails the
    set is halved (2+2, then singles): the passing half lands, then the other half is tested on top of what landed. `refuse(pr)` names a PR that fails alone."""
    if not prs:
        return
    if passes(prs):
        for pr in prs:
            land(pr)
        return
    if len(prs) == 1:
        refuse(prs[0])
        return
    middle = len(prs) // 2
    batch(prs[:middle], passes, land, refuse)
    batch(prs[middle:], passes, land, refuse)


def merge_batch(numbers: list[str], known: dict[str, str] | None = None) -> int:
    """`parity_squash.py merge-batch N1..N4`: the PRs in order, one smoke on the base with all of them merged in, each squashed with its head lease."""
    known = known or {}
    infos = {}
    for number in numbers:
        info = json.loads(subprocess.run(["gh", "pr", "view", number, "--json", "headRefOid,title,body,baseRefName"], capture_output=True, text=True, check=True).stdout)
        if info["baseRefName"] != "feat/parity":
            raise SystemExit(f"#{number} targets {info['baseRefName']}, not feat/parity")
        infos[number] = info
    landed, refused = [], []

    def passes(group: list[str]) -> bool:
        git("fetch", "-q", "origin", "feat/parity")
        git("checkout", "-q", "--detach", BASE)
        for number in group:
            git("fetch", "-q", "origin", infos[number]["headRefOid"])
            merge = subprocess.run(["git", "merge", "--no-edit", "-q", infos[number]["headRefOid"]], cwd=ROOT, capture_output=True, text=True)
            if merge.returncode:
                subprocess.run(["git", "merge", "--abort"], cwd=ROOT, capture_output=True)
                print(f"#{number}: does not merge with the others: {merge.stdout.strip()[:120] or merge.stderr.strip()[:120]}")
                return False
        for number in group:
            if why := refusals(git("rev-parse", "HEAD").strip(), infos[number]["body"] or ""):
                print(f"#{number}: " + "; ".join(why))
                return False
        if stale := ensure_bins():
            print(f"REFUSED: {stale}")
            return False
        merged_failed, last = smoke_failures(known)
        print(f"{' '.join('#' + n for n in group)}: {last}")
        why = subset_refusals(head_failures(git("rev-parse", BASE).strip()), merged_failed, any(claims_fix(infos[n]["body"] or "") for n in group))
        if why:
            print("\n".join(f"REFUSED: {w}" for w in why))
        return not why

    def land(number: str) -> None:
        done = subprocess.run(["gh", "pr", "merge", number, "--squash", "--match-head-commit", infos[number]["headRefOid"], "--subject", infos[number]["title"], "--body", known_red_line(known)])
        (landed if done.returncode == 0 else refused).append(number)
        if done.returncode == 0:
            git("fetch", "-q", "origin", "feat/parity")
            log_landed(git("rev-parse", BASE).strip(), number, "gate-ok")

    with land_lock():
        if git("status", "--porcelain", "--untracked-files=no").strip():
            raise SystemExit("this checkout has uncommitted changes")
        batch(list(numbers), passes, land, lambda number: refused.append(number))
    print(f"LANDED: {' '.join('#' + n for n in landed) or '-'}  REFUSED: {' '.join('#' + n for n in refused) or '-'}")
    return 1 if refused else 0


def claims_fix(description: str) -> bool:
    """Whether the PR says it fixes a red: a line `Fixes red` / `Fixes a red ...` in its description."""
    return bool(FIXES_RED.search(description or ""))


def subset_refusals(head: set[str], merged: set[str], fixes: bool) -> list[str]:
    """Why the merged tree may not land on a head that fails `head`: it fails a check the head passes, or (a PR that says it fixes a red) fails as many as the head.
    The gate must not deadlock on a red head, and must not let a PR add a red. Both sets are in the reason."""
    shown = f"head fails {{{', '.join(sorted(head)) or '-'}}}, merged tree fails {{{', '.join(sorted(merged)) or '-'}}}"
    if new := sorted(merged - head):
        return [f"the merged tree fails {', '.join(new)}, which the head passes ({shown})"]
    if fixes and head and len(merged) >= len(head):
        return [f"the PR says it fixes a red but the merged tree fails as many checks as the head ({shown})"]
    return []


def smoke_failures(known: dict[str, str] | None = None) -> tuple[set[str], str]:
    """(the failing checks, the smoke's last line) of parity_smoke on this checkout: its `FAILING:` line."""
    smoke = subprocess.run(
        ["uv", "run", "-q", "--project", "tools", "python", "tools/parity_smoke.py", "--bin", smoke_bins(), "--base", BASE, *[a for step in (known or {}) for a in ("--skip", step)]],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    lines = smoke.stdout.strip().splitlines()
    if smoke.returncode and not any(line.startswith("FAILING:") for line in lines):
        tail = " / ".join((smoke.stderr.strip().splitlines() or ["no output"])[-3:])[:400]
        return {"smoke-did-not-finish"}, f"the smoke exited {smoke.returncode} with no summary line; stderr: {tail}"
    names = next((line[len("FAILING:"):].strip() for line in reversed(lines) if line.startswith("FAILING:")), None)
    if names is None:
        names = "smoke-did-not-finish" if smoke.returncode else ""
    return {n for n in names.split(",") if n}, (lines or [""])[-1]


def head_failures(sha: str, compute=None, cache: Path = HEAD_FAILURES) -> set[str]:
    """The failing checks of the fast set on the head `sha`, computed once per sha (`compute()` on a checkout of it with ITS binaries; `head-failures` stores it)."""
    file = cache / f"{sha}.json"
    if file.exists():
        return set(json.loads(file.read_text()))
    if compute is None:
        raise SystemExit(f"no failure set is cached for the head {sha[:9]}: check the head out, build its binaries, set PARITY_BIN to them and run `tools/parity_squash.py head-failures`")
    failed = compute()
    cache.mkdir(parents=True, exist_ok=True)
    file.write_text(json.dumps(sorted(failed)))
    return failed


def log_landed(sha: str, number: str, result: str, path: Path | None = None) -> None:
    """One line per landed squash for the coordinator's log: `<sha> <PR> <gate result> <date>`."""
    path = path or LANDED_LOG
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "a") as log:
        log.write(f"{sha} #{number} {result} {datetime.date.today().isoformat()}\n")


def unlogged(log: Path, commits: list[str]) -> list[str]:
    """The commits (sha prefixes of the first-parent history since the cut, the log's first line) with no line in the log."""
    seen = set()
    for line in log.read_text().splitlines() if log.exists() else []:
        if line.strip():
            seen.add(line.split()[0])
    return [c for c in commits if not any(c.startswith(x) or x.startswith(c) for x in seen)]


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
    if stale := ensure_bins():
        print(f"REFUSED: {stale}")
        return 1
    merged_failed, last = smoke_failures(known)
    print(last)
    head_failed = head_failures(git("rev-parse", BASE).strip())
    if why := subset_refusals(head_failed, merged_failed, claims_fix(info["body"] or "")):
        print("\n".join(f"REFUSED: {w}" for w in why))
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
    log_landed(squash, number, "gate-ok" + (f" (head red: {','.join(sorted(head_failed))})" if head_failed else ""))
    if strays := squash_strays(squash, files):
        print(f"REVERT-DETECTED: squash {squash[:9]} of #{number} touches files the PR never listed (a stale tree reverting other work?); restore them NOW:")
        print("\n".join(f"  {path} {change}" for path, change in strays))
        return 3
    return 0


def main() -> int:
    if len(sys.argv) >= 3 and sys.argv[1] == "merge":
        rest = sys.argv[3:]
        return merge(sys.argv[2], parse_known_red([rest[i + 1] for i, a in enumerate(rest) if a == "--known-red" and i + 1 < len(rest)]))
    if len(sys.argv) >= 2 and sys.argv[1] == "head-failures":
        sha = git("rev-parse", "HEAD").strip()
        (HEAD_FAILURES / f"{sha}.json").unlink(missing_ok=True)
        failed = head_failures(sha, lambda: smoke_failures()[0])
        print(f"{sha[:9]}: fails {{{', '.join(sorted(failed)) or '-'}}}")
        return 0
    if len(sys.argv) >= 2 and sys.argv[1] == "unlogged":
        cut = LANDED_LOG.read_text().splitlines()[0].split()[0] if LANDED_LOG.exists() else BASE
        shas = git("log", "--first-parent", "--format=%h", f"{cut}..{BASE}").split()
        missing = unlogged(LANDED_LOG, shas)
        for sha in missing:
            print(f"UNLOGGED {sha} {git('log', '-1', '--format=%an: %s', sha).strip()[:90]}")
        return 1 if missing else 0
    if len(sys.argv) >= 3 and sys.argv[1] == "merge-batch":
        numbers = [a for a in sys.argv[2:] if not a.startswith("--") and a.isdigit()]
        rest = sys.argv[2:]
        return merge_batch(numbers, parse_known_red([rest[i + 1] for i, a in enumerate(rest) if a == "--known-red" and i + 1 < len(rest)]))
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
