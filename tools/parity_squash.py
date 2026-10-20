#!/usr/bin/env python3
"""The one way to squash a pull request into feat/parity: a bare `gh pr merge` merged stale heads that reverted other PRs' work (#2329 reverted decide's LVI step).

    [PARITY_BIN=DIR] tools/parity_squash.py merge PR [--known-red STEP=ISSUE]...   under /tmp/land.lock (land.sh's): merge feat/parity into the PR head here (not pushed), refuse or run the fast set on THAT, then `gh pr merge --squash --match-head-commit`
    tools/parity_squash.py merge-batch N1..N8 [--known-red STEP=ISSUE]...   up to eight ready PRs: ONE fast set on the base with all merged in; on failure halve (the passing half lands first)
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
import time
from concurrent.futures import ThreadPoolExecutor
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


sys.path.insert(0, str(Path(__file__).resolve().parent))
from stages import PREFIX, stage  # noqa: E402


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


DOCS_PAGE = re.compile(r"^> Generated from the `//!` headers? of ")


def is_generated(base: str, path: str, cwd: Path = ROOT) -> bool:
    """Whether `path` at `base` is a docsbuild page: the docs check regenerates it on the merged tree, so a rewrite of its source is not a deletion."""
    done = subprocess.run(["git", "show", f"{base}:{path}"], cwd=cwd, capture_output=True, text=True)
    return done.returncode == 0 and bool(DOCS_PAGE.match(done.stdout.split("\n", 1)[0]))


def losses(base: str, head: str, cwd: Path = ROOT) -> list[tuple[str, int, int, bool]]:
    """(path, deleted lines, test functions removed, whole file gone) for each file of the net diff base..head that loses more than LIMIT lines, loses a test or goes.
    Own commits or not: #2443's own commit deleted scev/expand_iv.rs (855 lines) from a stale tree, and 'outside its own commits' let it through."""
    out = []
    limit = 10**9 if os.environ.get("LAND_FAST") == "1" else LIMIT  # fast mode merges onto the head: a net line loss is the PR's own refactor; whole files and tests still count
    for line in git("diff", "--numstat", base, head, cwd=cwd).splitlines():
        added, gone, path = line.split("\t", 2)
        if gone == "-" or is_generated(base, path, cwd):
            continue
        diff = git("diff", base, head, "--", path, cwd=cwd)
        tests = len(removed_tests(diff))
        whole = bool(re.search(r"^deleted file mode", diff, re.M))
        if int(gone) > limit or tests or whole:
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


PRECHECK_CACHE = LANDED_LOG.parent / "precheck-cache.json"  # (PR head sha, feat/parity sha) -> the verdict line, and the head's own check failures per sha: ~40 candidates per cycle must not rebuild the dev checkout each
PRECHECK_DIR = Path(os.environ.get("PRECHECK_DIR", Path.home() / "scratch/precheck"))  # a persistent dev checkout + target dir: no dist build, seconds per PR after the first


PRECHECK_DIR = Path(os.environ.get("PRECHECK_DIR", Path.home() / "scratch/precheck"))  # a persistent dev checkout + target dir: no dist build, seconds per PR after the first


def fmt_differs(root: Path, files: list[str], run=subprocess.run) -> set[str]:
    """The `.rs` files of `files` that `tools/fmt.sh --stdin` would change: --check on only the PR's own files, ~1 s where the whole tree takes ~13."""

    def one(path: str) -> str | None:
        text = (root / path).read_text()
        done = run(["tools/fmt.sh", "--stdin", path], cwd=root, input=text, capture_output=True, text=True)
        return path if done.returncode or done.stdout != text else None

    rust = [f for f in files if f.endswith(".rs") and (root / f).is_file()]
    with ThreadPoolExecutor(max_workers=8) as pool:
        return {path for path in pool.map(one, rust) if path}


def precheck_checks(dev: Path, target: Path):
    """The cheap static checks of the gate, each `(name, root -> {check: the set of items failing})`: a ratchet's name, fmt's files, the docs' stale pages. Run on the
    dev checkout against the persistent dev target; the items are what a head's own failures are told apart from a PR's by."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import parity_smoke  # noqa: PLC0415
    from buildslot import slotted  # noqa: PLC0415

    env = {**os.environ, "CARGO_TARGET_DIR": str(target)}

    def ratchets(root: Path) -> dict[str, set[str]]:
        failed = parity_smoke.ratchet_failures(root, run=lambda command: subprocess.run(slotted(command), cwd=root, env=env, capture_output=True, text=True))
        return {name: {name} for name in failed}

    def fmt(root: Path, only: list[str] | None = None) -> dict[str, set[str]]:
        if only is not None:  # the PR side: its own files only (the baseline, computed once per feat/parity sha, checks them all)
            files = fmt_differs(root, only)
            return {"fmt": files} if files else {}
        done = subprocess.run(["tools/fmt.sh", "--check"], cwd=root, env=env, capture_output=True, text=True)
        files = {line.strip() for line in done.stdout.splitlines() if re.fullmatch(r"\S+\.(rs|py|toml)", line.strip())}
        return {"fmt": files or {"fmt --check failed"}} if done.returncode else {}

    def docs(root: Path) -> dict[str, set[str]]:
        done = subprocess.run([sys.executable, "tools/docsbuild/build.py", "--check"], cwd=root, env=env, capture_output=True, text=True)
        if not done.returncode:
            return {}
        out = (done.stdout + done.stderr).splitlines()
        return {"docs": {line.strip() for line in out if line.strip().startswith("docs/")} or {"docsbuild --check failed"}}

    fmt.takes_only = True
    return [("ratchets", ratchets), ("fmt", fmt), ("docs", docs)]


def precheck_items(dev: Path, checks, deletions_of=None, seconds: dict[str, float] | None = None, only: list[str] | None = None) -> dict[str, set[str]]:
    """Every check's failing items on the checkout in `dev`, merged by check name; `seconds` collects each check's wall time; `only` (the PR's changed
    files) goes to the checks that can use it (`takes_only`)."""
    found: dict[str, set[str]] = {}
    for label, check in checks:
        started = time.monotonic()
        for name, items in (check(dev, only=only) if only is not None and getattr(check, "takes_only", False) else check(dev)).items():
            found.setdefault(name, set()).update(items)
        if seconds is not None:
            seconds[label] = round(time.monotonic() - started, 1)
    return found


def precheck_baseline(dev: Path, checks, base_sha: str, cache: Path) -> dict[str, set[str]]:
    """The checks' failures on feat/parity ITSELF, cached per its sha: a red head file (fmt, a stale page) must not refuse every PR, nor the PR that fixes it."""
    cached = json.loads(cache.read_text()) if cache.exists() else {}
    key = f"base {base_sha}"
    if key in cached:
        return {name: set(items) for name, items in cached[key].items()}
    git("checkout", "-q", "--detach", base_sha, cwd=dev)
    found = precheck_items(dev, checks)
    cache.parent.mkdir(parents=True, exist_ok=True)
    cache.write_text(json.dumps({**cached, key: {name: sorted(items) for name, items in found.items()}}))
    return found


def precheck(number: str, dev: Path = PRECHECK_DIR / "wt", target: Path = PRECHECK_DIR / "target", checks=None, merge=None, deletions=None, info=None, head_failed: set[str] | None = None, cache: Path = PRECHECK_CACHE, base_sha: str | None = None) -> str:
    """`N: PASS`, or `N: REFUSED: <names>` after merging PR N's head with feat/parity in the dev checkout and running the cheap static checks (ratchets, fmt, docs, the
    deletion rule) there: seconds, no dist build. #2836 cost a batch 26 minutes of halving for a failure these checks name. Only items NEW against feat/parity's own
    failures refuse (by check AND file/page): a red head file refused every PR, the fixes included. The verdict is cached per (PR head sha, feat/parity sha)."""
    info = info or json.loads(subprocess.run(["gh", "pr", "view", number, "--json", "headRefOid,body,baseRefName"], capture_output=True, text=True, check=True).stdout)
    if info["baseRefName"] != "feat/parity":
        return f"{number}: REFUSED: targets {info['baseRefName']}, not feat/parity"
    base_sha = base_sha or git("rev-parse", BASE).strip()
    key = f"{info['headRefOid']} {base_sha}"
    cached = json.loads(cache.read_text()) if cache.exists() else {}
    if key in cached:
        return cached[key].replace("{number}", number)
    line = _precheck(number, info, dev, target, checks, merge, deletions, head_failed, cache, base_sha)
    cache.parent.mkdir(parents=True, exist_ok=True)
    cached = json.loads(cache.read_text()) if cache.exists() else {}
    cache.write_text(json.dumps({**cached, key: line.replace(number, "{number}", 1)}))
    return line


def _precheck(number, info, dev, target, checks, merge, deletions, head_failed, cache, base_sha) -> str:
    if not dev.exists():
        dev.parent.mkdir(parents=True, exist_ok=True)
        git("fetch", "-q", "origin", "feat/parity")
        git("worktree", "add", "-q", "--detach", str(dev), base_sha)
    git("fetch", "-q", "origin", info["headRefOid"], cwd=dev)
    checks = list(checks) if checks is not None else precheck_checks(dev, target)
    before = precheck_baseline(dev, checks, base_sha, cache)
    seconds: dict[str, float] = {}
    started = time.monotonic()
    sha, why = (merge or merged_result)(info["headRefOid"], BASE, dev)
    seconds["merge"] = round(time.monotonic() - started, 1)
    if sha is None:
        return f"{number}: REFUSED: {why}"
    after = precheck_items(dev, checks, seconds=seconds, only=git("diff", "--name-only", base_sha, sha, cwd=dev).split())
    with (cache.parent / "precheck-times.log").open("a") as log:  # where a precheck's minutes go, per PR
        log.write(json.dumps({"pr": number, "at": datetime.datetime.now().strftime("%H:%M:%S"), "load": round(os.getloadavg()[0], 1), **seconds}) + "\n")
    if problems := (deletions or refusals)(sha, info["body"] or "", BASE, dev):
        after["deletes"] = {problem.split(":")[0] for problem in problems}
    new = {name: items - before.get(name, set()) for name, items in after.items() if name not in (head_failed or set())}
    new = {name: items for name, items in new.items() if items}
    if not new:
        return f"{number}: PASS"
    return f"{number}: REFUSED: {', '.join(sorted(new))} ({'; '.join(f'{k}: ' + ', '.join(sorted(v)[:6]) for k, v in sorted(new.items()))})"


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

    stage("build")
    directory = Path(smoke_bins(env))
    if not llrmbin.stale_binaries(directory, cwd):
        return ""
    built = (build or (lambda: subprocess.run(slotted(["cargo", "build", "--release", "--bins", "-q"]), cwd=cwd, env={**os.environ, **(env or {})}, capture_output=True, text=True)))()
    still = llrmbin.stale_binaries(directory, cwd)
    if not still:
        return ""
    # A stale binary after a rebuild is a build that failed (its exit code and last lines) or one that left the binaries older than a source: say which.
    ran = f"; the rebuild exited {built.returncode}: {' / '.join((built.stderr or built.stdout or '').strip().splitlines()[-3:])[:300]}" if getattr(built, "returncode", 0) else ""
    detail = llrmbin.stale_detail(directory, cwd)
    return f"binaries still older than the merged tree after a rebuild: {', '.join(still)} in {directory}{'; ' + detail if detail else ''}{ran}"


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


GENERATED = ("docs/optimizations/passes/", "docs/optimizations/analyses/", "docs/optimizations/backend/", "docs/architecture/readme.md")  # what tools/docsbuild/build.py --sync writes


def regenerate(conflicts: list[str], cwd: Path = ROOT) -> str:
    """"" when every conflicted path is generated and `build.py --sync --force` on the merged tree made it current (then `--check` agrees) and the merge was committed;
    else why not. Two PRs that each add a page both rewrite the index: a hand merge of generated text is what blocked the decide flip, the LSR stack and indvars."""
    if not conflicts or not all(path.startswith(GENERATED) for path in conflicts):
        return "conflicts in files nothing generates: " + ", ".join(path for path in conflicts if not path.startswith(GENERATED))
    for path in conflicts:
        git("checkout", "--ours", "--", path, cwd=cwd)
    build = [sys.executable, "tools/docsbuild/build.py"]
    for flags in (["--sync", "--force"], ["--check"]):
        done = subprocess.run([*build, *flags], cwd=cwd, capture_output=True, text=True)
        if done.returncode:
            return f"docsbuild {' '.join(flags)} failed on the merged tree: {(done.stdout + done.stderr).strip().splitlines()[-1:] or ['no output']}"
    git("add", "-A", "docs", cwd=cwd)
    done = subprocess.run(["git", "-c", "user.name=parity_squash", "-c", "user.email=parity_squash@localhost", "commit", "-q", "--no-edit"], cwd=cwd, capture_output=True, text=True)
    return "" if done.returncode == 0 else f"committing the regenerated merge failed: {done.stderr.strip()[:200]}"


def merged_result(head: str, base: str = BASE, cwd: Path = ROOT) -> tuple[str | None, str]:
    """(sha, "") of `head` with `base` merged into it, checked out detached in `cwd` and not pushed; (None, why) when `base` conflicts. When `head` already
    contains `base` the result is `head` itself. The squash lands on whatever feat/parity is at the call: the tests must run on this result, not on a head that
    compiled beside a base that has since moved (#2544 and #2564 each compiled; together the head did not build for three hours). A conflict only in generated
    files is resolved by regenerating them (`regenerate`)."""
    stage("merge")
    git("checkout", "-q", "--detach", head, cwd=cwd)
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=cwd).returncode:
        merged = subprocess.run(["git", "merge", "--no-edit", "-q", base], cwd=cwd, capture_output=True, text=True)
        if merged.returncode:
            conflicts = git("diff", "--name-only", "--diff-filter=U", cwd=cwd).split()
            if conflicts and all(path.startswith(GENERATED) for path in conflicts):
                if not (why := regenerate(conflicts, cwd)):
                    return git("rev-parse", "HEAD", cwd=cwd).strip(), ""
                subprocess.run(["git", "merge", "--abort"], cwd=cwd, capture_output=True)
                return None, f"merging {base} into the PR head conflicts in {', '.join(conflicts)} (generated) and regenerating failed: {why}"
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


FAST = os.environ.get("LAND_FAST") == "1"  # the user's call (2026-10-19): no smoke, no tests, no precheck; each PR is merged in order and squashed; only a build of the combined tree can halve a batch
MAX_BATCH = 16 if FAST else 8  # PRs per smoke: with ~38 ready PRs queued, batches of four took 12-16 minutes each; a failing batch is halved, so the cost of a bad PR is log2(8) more smokes


def batch(prs: list, passes, land, refuse) -> None:
    """Land `prs` (up to MAX_BATCH, ready, in order) with ONE test of the combined tree. `passes(prs)` merges them all into the base and runs the fast set; when it fails the
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


def fast_merge(numbers: list[str], merge_one) -> tuple[list[str], dict[str, str]]:
    """The PRs of a fast batch, in order: `merge_one(number)` merges one into the checkout and returns `""` or why it cannot join (a conflict with an earlier one, a deletion it
    does not name). A PR that cannot join is left out, with its reason, and never fails the others."""
    kept, left_out = [], {}
    for number in numbers:
        why = merge_one(number)
        if why:
            left_out[number] = why
        else:
            kept.append(number)
    return kept, left_out


def merge_batch(numbers: list[str], known: dict[str, str] | None = None) -> int:
    """`parity_squash.py merge-batch N1..N8`: the PRs in order, one smoke on the base with all of them merged in, each squashed with its head lease."""
    known = known or {}
    if len(numbers) > MAX_BATCH:
        raise SystemExit(f"merge-batch takes at most {MAX_BATCH} PRs per smoke, got {len(numbers)}: split the queue")
    infos = {}
    for number in numbers:
        info = json.loads(subprocess.run(["gh", "pr", "view", number, "--json", "headRefOid,title,body,baseRefName"], capture_output=True, text=True, check=True).stdout)
        if info["baseRefName"] != "feat/parity":
            raise SystemExit(f"#{number} targets {info['baseRefName']}, not feat/parity")
        infos[number] = info
    landed, refused = [], []

    left_out: dict[str, str] = {}

    def passes_fast(group: list[str]) -> bool:
        git("fetch", "-q", "origin", "feat/parity")
        git("checkout", "-q", "--detach", BASE)

        def merge_one(number: str) -> str:
            before = git("rev-parse", "HEAD").strip()
            git("fetch", "-q", "origin", infos[number]["headRefOid"])
            merge = subprocess.run(["git", "merge", "--no-edit", "-q", infos[number]["headRefOid"]], cwd=ROOT, capture_output=True, text=True)
            if merge.returncode and (conflicts := git("diff", "--name-only", "--diff-filter=U").split()) and not regenerate(conflicts):
                merge = subprocess.CompletedProcess([], 0)  # only generated docs conflicted: regenerated and committed
            if merge.returncode:
                subprocess.run(["git", "merge", "--abort"], cwd=ROOT, capture_output=True)
                return "conflicts with an earlier PR of the batch or with feat/parity: its owner merges fresh feat/parity"
            if why := refusals(git("rev-parse", "HEAD").strip(), infos[number]["body"] or "", base=before):
                git("reset", "-q", "--hard", before)
                return "; ".join(why)
            return ""

        kept, out = fast_merge(group, merge_one)
        left_out.update(out)
        for number, why in out.items():
            print(f"#{number}: left out: {why}")
        if not kept:
            return True
        if broken := _workspace_check(ROOT):
            print(f"REFUSED: the batch does not build: {broken[:300]}")
            return False
        return True

    def passes(group: list[str]) -> bool:
        if FAST:
            return passes_fast(group)
        git("fetch", "-q", "origin", "feat/parity")
        head_failed = head_failures(git("rev-parse", BASE).strip(), lambda: compute_head_failures(known))
        git("checkout", "-q", "--detach", BASE)
        for number in group:
            git("fetch", "-q", "origin", infos[number]["headRefOid"])
            merge = subprocess.run(["git", "merge", "--no-edit", "-q", infos[number]["headRefOid"]], cwd=ROOT, capture_output=True, text=True)
            if merge.returncode and (conflicts := git("diff", "--name-only", "--diff-filter=U").split()) and not regenerate(conflicts):
                continue  # only generated docs conflicted: regenerated and committed
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
        why = subset_refusals(head_failed, merged_failed, any(claims_fix(infos[n]["body"] or "") for n in group))
        if why:
            print("\n".join(f"REFUSED: {w}" for w in why))
        return not why

    def land(number: str) -> None:
        if number in left_out:
            refused.append(number)
            return
        stage(f"squash #{number}")
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


def unit_tests_of(failures: set[str]) -> dict[str, set[str]]:
    """crate -> test names, of the `unit <crate>::<test>` entries of a failure set."""
    found: dict[str, set[str]] = {}
    for name in failures:
        if name.startswith("unit ") and "::" in name:
            crate, test = name[len("unit "):].split("::", 1)
            found.setdefault(crate, set()).add(test)
    return found


def head_expanded(head: set[str], merged: set[str], run_on_head) -> set[str]:
    """`head` plus the unit tests the merged tree fails that the head fails too. The smoke runs unit tests only for the crates a merge touches, so the head's cached set
    holds none: a test inside an already-red crate is new only when it passes on the head. `run_on_head(crate, tests)` -> the tests of `tests` that fail there."""
    extra = set()
    for crate, tests in unit_tests_of(merged - head).items():
        extra |= {f"unit {crate}::{test}" for test in run_on_head(crate, tests)}
    return head | extra


def failing_on_checkout(crate: str, tests: set[str], cwd: Path = ROOT) -> set[str]:
    """Which of `tests` of `crate` fail on the checkout in `cwd` (exact names, release not needed: the smoke ran them as `cargo test -q --lib`)."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import parity_smoke
    from buildslot import slotted  # noqa: PLC0415

    done = subprocess.run(slotted(["cargo", "test", "-q", "--lib", "-p", crate, "--", "--exact", *sorted(tests)]), cwd=cwd, capture_output=True, text=True)
    return {name for name in tests if name in set(parity_smoke.failed_tests(done.stdout + done.stderr))}


def run_streaming(command: list[str], cwd: Path):
    """The finished process (.returncode, .stdout, .stderr) with the stage lines of its stdout echoed as they arrive, so the squash's log has the smoke's step times."""
    import tempfile
    import types

    with tempfile.TemporaryFile("w+") as err:
        process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=err, text=True)
        out = []
        for line in process.stdout:
            out.append(line)
            if f" {PREFIX} " in line:
                print(line.rstrip(), flush=True)
        process.wait()
        err.seek(0)
        return types.SimpleNamespace(returncode=process.returncode, stdout="".join(out), stderr=err.read())


def smoke_failures(known: dict[str, str] | None = None) -> tuple[set[str], str]:
    """(the failing checks, the smoke's last line) of parity_smoke on this checkout: its `FAILING:` line."""
    stage("smoke")
    smoke = run_streaming(
        ["uv", "run", "-q", "--project", "tools", "python", "tools/parity_smoke.py", "--bin", smoke_bins(), "--base", BASE, *[a for step in (known or {}) for a in ("--skip", step)]],
        ROOT,
    )
    lines = smoke.stdout.strip().splitlines()
    if smoke.returncode and not any(line.startswith("FAILING:") for line in lines):
        tail = " / ".join((smoke.stderr.strip().splitlines() or ["no output"])[-3:])[:400]
        return {"smoke-did-not-finish"}, f"the smoke exited {smoke.returncode} with no summary line; stderr: {tail}"
    names = next((line[len("FAILING:"):].strip() for line in reversed(lines) if line.startswith("FAILING:")), None)
    if names is None:
        names = "smoke-did-not-finish" if smoke.returncode else ""
    return {n for n in names.split(",") if n}, (lines or [""])[-1]


def head_failures_cached() -> set[str]:
    """The cached failure set of the current head, or none known (then every failure refuses)."""
    file = HEAD_FAILURES / f"{git('rev-parse', BASE).strip()}.json"
    return set(json.loads(file.read_text())) if file.exists() else set()


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
    stage(f"log #{number}")
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


def compute_head_failures(known: dict[str, str] | None = None) -> set[str]:
    """The failing checks on the head itself: check origin/feat/parity out, build its binaries when stale, run the smoke. Slow (a build), once per head sha."""
    stage("head-failures")
    git("checkout", "-q", "--detach", BASE)
    if stale := ensure_bins():
        raise SystemExit(f"the head's own binaries: {stale}")
    return smoke_failures(known)[0]


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
    head_failed = head_failures(tested_base, lambda: compute_head_failures(known))
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
    if unit_tests_of(merged_failed - head_failed):
        git("checkout", "-q", "--detach", BASE)
        head_failed = head_expanded(head_failed, merged_failed, failing_on_checkout)
        git("checkout", "-q", "--detach", merged)
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
    """The landing commands run at the landing's build priority (`buildslot.landing`); the others (audit, unlogged) start no cargo."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from buildslot import landing  # noqa: PLC0415

    with landing():
        return dispatch()


def dispatch() -> int:
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
    if len(sys.argv) >= 3 and sys.argv[1] == "precheck":
        line = precheck(sys.argv[2], head_failed=head_failures_cached())
        print(line)
        return 0 if line.endswith("PASS") else 1
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
