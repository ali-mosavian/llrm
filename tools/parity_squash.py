#!/usr/bin/env python3
"""The one way to squash a pull request into feat/parity: a bare `gh pr merge` merged stale heads that reverted other PRs' work (#2329 reverted decide's LVI step).

    [PARITY_BIN=DIR] tools/parity_squash.py merge PR [--known-red STEP=ISSUE]...   under /tmp/land.lock (land.sh's): merge feat/parity into the PR head here (not pushed), refuse or run the fast set on THAT, then `gh pr merge --squash --match-head-commit`
    tools/parity_squash.py merge-batch N1..N8 [--known-red STEP=ISSUE]...   up to eight ready PRs: ONE fast set on the base with all merged in; on failure halve (the passing half lands first)
    tools/parity_squash.py audit [SINCE] every squash on origin/feat/parity since SINCE (default 96468410e): what it deleted without a Deletes:/Reverts: line in its PR's description

Subset rule: the fast set runs on the head (origin/feat/parity, failure set cached per sha by `head-failures`) AND on the merged tree; the merged tree may fail only checks the head fails, and strictly fewer
when the description has a `Fixes red` line; both sets are in the refusal. Every landing appends `<sha> #PR gate-ok <date>` to ~/scratch/coordwork/landed.log; `unlogged` lists feat/parity commits with no line.
Refused unless: the PR head contains the current origin/feat/parity; the squash deletes no file a docs/*.md still links to, and no file, no test function (`#[test] fn`, `def test_`, unless re-added) and no more than
LIMIT lines of a file, unless a `Deletes:` or `Reverts:` line of the description names the path (own commits or not: #2443's deleted scev/expand_iv.rs); parity_smoke passes on that exact head.
PARITY_BIN names the directory of the built release llrm-cc, llrm-llc, llrm-qb and llrm-nib (parity_smoke needs them); unset, it is the repo target dir's release
(LLRM_BIN / CARGO_TARGET_DIR as tools/llrmbin.py reads them) when all three are there.
`--known-red sc=2438` skips ONLY that smoke step (named, with its issue), still runs every other check, and puts one line in the squash's body:
`Known red: sc (#2438)`.
"""
from __future__ import annotations

import contextlib
import datetime
import fcntl
import hashlib
import importlib
import json
import os
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import threading
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
from stages import PREFIX, stage, summary as stage_summary  # noqa: E402
import pidfile  # noqa: E402


MERGE_DRIVER = "python3 tools/merge_tests_append.py %O %A %B"  # .gitattributes names it for the tests files; a git config is per clone, so every git below gets it here


def with_merge_driver(env: dict[str, str]) -> None:
    count = int(env.get("GIT_CONFIG_COUNT", "0"))
    env[f"GIT_CONFIG_KEY_{count}"] = "merge.tests-append.driver"
    env[f"GIT_CONFIG_VALUE_{count}"] = MERGE_DRIVER
    env["GIT_CONFIG_COUNT"] = str(count + 1)


with_merge_driver(os.environ)


def with_pinned_clang(env: dict[str, str], root: Path, by_sha: Path | None = None) -> None:
    """LLRM_CLANG_M16 is the clang built at the m16 sha of `root`'s toolchain/clang/pin (its second line), found at <by_sha>/<sha>/bin/clang;
    where no such build is there the variable stays as the runner has it. A batch that bumps the pin changes the clang its tests need."""
    by_sha = by_sha or Path(env.get("CLANG_BY_SHA") or Path.home() / "scratch/clang-by-sha")
    lines = (root / "toolchain/clang/pin").read_text().splitlines() if (root / "toolchain/clang/pin").exists() else []
    if len(lines) > 1 and (clang := by_sha / lines[1].split()[0] / "bin/clang").exists():
        env["LLRM_CLANG_M16"] = str(clang)


RATE_LIMITED = ("API rate limit exceeded", "rate limit exceeded")


def rate_limited(text: str) -> bool:
    """Whether `gh` failed on GitHub's quota (HTTP 403 or 429), not on the PR or the call."""
    return any(one in text for one in RATE_LIMITED)


def rate_limit_wait(cap: float = 3700) -> float:
    """Sleeps until the core quota resets (`gh api rate_limit` costs none; 60 s when it cannot be read) and returns the seconds slept. A spent quota is nobody's refusal: the call waits and goes again."""
    done = subprocess.run(["gh", "api", "rate_limit", "-q", ".resources.core.reset"], capture_output=True, text=True, errors="replace")
    try:
        wait = int(done.stdout.strip()) - time.time()
    except ValueError:
        wait = 60
    wait = min(max(wait, 5) + 2, cap)
    print(f"gh: rate limit spent; waiting {wait:.0f} s for the reset", file=sys.stderr)
    time.sleep(wait)
    return wait


def rate_limit_proof(run=subprocess.run, tries: int = 3):
    """`run` that, when `gh` fails on the rate limit, waits for the reset and runs the call again (`tries` times) instead of handing the failure on as a refusal or a crash."""

    def proofed(cmd, **kwargs):
        done = run(cmd, **kwargs)
        for _ in range(tries):
            if not done.returncode or not rate_limited(f"{done.stderr}{done.stdout}"):
                break
            rate_limit_wait()
            done = run(cmd, **kwargs)
        return done

    return proofed


def gh_run(cmd, **kwargs):
    """`subprocess.run` for a `gh` call that waits out the rate limit (`check=True` raises only after that)."""
    check = kwargs.pop("check", False)
    done = rate_limit_proof(subprocess.run)(cmd, **kwargs)
    if check:
        done.check_returncode()
    return done


def git(*args: str, cwd: Path = ROOT) -> str:
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, errors="replace")
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
    done = subprocess.run(["git", "show", f"{base}:{path}"], cwd=cwd, capture_output=True, text=True, errors="replace")
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


# The user's rule: nothing in the repository, its PRs or its history names the assistant that wrote it. Spelled in two halves so this file does not match itself.
ASSISTANT_NAME = re.compile(r"\b(?:clau" + "de|anthro" + r"pic)\b", re.I)


def assistant_mentions(title: str, description: str, base: str, head: str, cwd: Path = ROOT) -> list[str]:
    """Where the PR names the assistant: its title, its body, a commit message of base..head, or a line the squash adds."""
    why = []
    for where, text in (("title", title), ("description", description)):
        if ASSISTANT_NAME.search(text or ""):
            why.append(f"the {where} names the assistant; remove it")
    for sha, message in (line.split("\t", 1) for line in git("log", "--format=%h\t%s%n%b%x00", f"{base}..{head}", cwd=cwd).split("\0") if "\t" in line):
        if ASSISTANT_NAME.search(message):
            why.append(f"commit {sha.strip()} names the assistant in its message; amend it out")
    path = ""
    for line in git("diff", "-U0", base, head, cwd=cwd).splitlines():
        if line.startswith("+++ "):
            path = line[6:]
        elif line.startswith("+") and ASSISTANT_NAME.search(line):
            why.append(f"{path}: an added line names the assistant: {line[1:80].strip()}")
            break
    return why


MD_LINK = re.compile(r"\]\(([^)#\s]+)(?:#[^)]*)?\)")


def dead_links(base: str, head: str, cwd: Path = ROOT) -> list[tuple[str, str]]:
    """(deleted path, docs page) for each file the squash deletes that a docs/*.md at `head` still links to. Two PRs deleted the dse, unroll and peel pages and left their
    readme links; the docs_links test went red on feat/parity for hours."""
    gone = git("diff", "--name-only", "--diff-filter=D", base, head, cwd=cwd).split()
    found = []
    for path in gone:
        listed = subprocess.run(["git", "grep", "-l", "-F", "-e", Path(path).name, head, "--", "docs"], cwd=cwd, capture_output=True, text=True, errors="replace").stdout.split()
        for entry in listed:
            page = entry.split(":", 1)[1]
            if not page.endswith(".md"):
                continue
            text = git("show", f"{head}:{page}", cwd=cwd)
            if any(os.path.normpath(os.path.join(os.path.dirname(page), target)) == path for target in MD_LINK.findall(text) if "://" not in target):
                found.append((path, page))
    return found


PERF_TITLE = re.compile(r"^\s*perf(\(|:)", re.I)
REFACTOR_TITLE = re.compile(r"^\s*refactor(\(|:)", re.I)
PASS_CODE = re.compile(r"^crates/(opt/[^/]+|codegen/llrm-codegen)/src/(?!.*(?:_tests\.rs|/tests/)).*\.rs$")
FEAT_TITLE = re.compile(r"^\s*feat(\(|:)", re.I)
PIPELINE_WIRING = "crates/opt/llrm-transforms/src/pipeline.rs"
ROWS_LINE = re.compile(r"^\s*Rows:\s*\S+", re.I | re.M)


def perf_needs_rows(title: str, description: str, files: list[str] | tuple[str, ...] = ()) -> list[str]:
    """A `perf` PR, or a `refactor` PR that changes pass code (non-test .rs under crates/opt or the backend), lands only with a `Rows:` line naming its rows pair
    (base and new binaries, and that the run diff is clean): #3251 landed without one and broke alias/farstruct.nib. A `feat` PR that wires a pass into pipeline.rs needs one too
    (#3344 turned LoopInterchange on at every level). Other `fix` and `feat` PRs keep the build check only."""
    pass_code = any(PASS_CODE.match(f) for f in files)
    wires_a_pass = bool(FEAT_TITLE.search(title or "")) and PIPELINE_WIRING in files
    if (PERF_TITLE.search(title or "") or wires_a_pass or (REFACTOR_TITLE.search(title or "") and pass_code)) and not ROWS_LINE.search(description or ""):
        return ["a perf PR, a feat PR that wires a pass into pipeline.rs, or a refactor PR that changes pass code, needs a `Rows:` line in the description naming its rows pair and run diff (bench's pair: base and new binaries, "
                "rows above base, NEW FAIL lines); mark the PR draft until the pair exists"]
    return []


MARKER = re.compile(r"^(<{7}|>{7})( |$)")


def conflict_markers(base: str, head: str, cwd: Path = ROOT) -> list[str]:
    """A file where an added line is a merge conflict marker (`<<<<<<<` or `>>>>>>>`): pushed to an open branch twice in a day, it would break every batch it joined."""
    path, found = "", []
    for line in git("diff", "-U0", base, head, cwd=cwd).splitlines():
        if line.startswith("+++ "):
            path = line[6:]
        elif line.startswith("+") and MARKER.match(line[1:]) and path not in found:
            found.append(path)
    return [f"{path}: an added line is a merge conflict marker; resolve the merge and push again" for path in found]


# The checks of the PR's description (the `Deletes:`/`Reverts:` paths line, the `Rows:` line of a perf PR) are WARNINGS while the backend parts land (the user's call);
# BODY_CHECKS=refuse makes them refusals again. A build or unit failure, a conflict and any line naming the assistant stay refusals.
BODY_CHECKS = os.environ.get("BODY_CHECKS", "warn")


def body_checks(head: str, description: str, base: str = BASE, cwd: Path = ROOT, title: str = "") -> list[str]:
    """What the description of the PR lacks: a deletion not named in `Deletes:`/`Reverts:`, a `Rows:` line a perf PR needs."""
    outside = set(outside_files(base, head, cwd))
    why = []
    for path, gone, tests, whole in losses(base, head, cwd):
        if not acknowledged(description, path):
            what = "deletes the whole file" if whole else f"deletes {gone} line(s)"
            why.append(f"{path}: the squash {what}{f' and {tests} test function(s)' if tests else ''}{' (no commit of this PR touched it)' if path in outside else ''}; "
                       "name the path in a `Deletes:` or `Reverts:` line of the description if that is meant")
    changed = subprocess.run(["git", "diff", "--name-only", f"{base}...{head}"], cwd=cwd, capture_output=True, text=True, errors="replace").stdout.split()
    return why + perf_needs_rows(title, description, changed)


def warnings(head: str, description: str, base: str = BASE, cwd: Path = ROOT, title: str = "") -> list[str]:
    """The description checks that only warn (all of them unless BODY_CHECKS=refuse)."""
    return [] if BODY_CHECKS == "refuse" else body_checks(head, description, base, cwd, title)


def refusals(head: str, description: str, base: str = BASE, cwd: Path = ROOT, title: str = "") -> list[str]:
    """Why this head may not be squashed into `base`; empty if it may."""
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=cwd).returncode:
        return [f"the PR head does not contain {base}: merge it in first (a stale head squashes other PRs' work away)"]
    why = [f"{page}: still links to {path}, which the squash deletes; remove or repoint the link in this PR" for path, page in dead_links(base, head, cwd)]
    soft = body_checks(head, description, base, cwd, title) if BODY_CHECKS == "refuse" else []
    return soft + why + conflict_markers(base, head, cwd) + assistant_mentions(title, description, base, head, cwd)


PRECHECK_CACHE = LANDED_LOG.parent / "precheck-cache.json"  # (PR head sha, feat/parity sha) -> the verdict line, and the head's own check failures per sha: ~40 candidates per cycle must not rebuild the dev checkout each
PRECHECK_DIR = Path(os.environ.get("PRECHECK_DIR", Path.home() / "scratch/precheck"))  # a persistent dev checkout + target dir: no dist build, seconds per PR after the first


PRECHECK_DIR = Path(os.environ.get("PRECHECK_DIR", Path.home() / "scratch/precheck"))  # a persistent dev checkout + target dir: no dist build, seconds per PR after the first


def fmt_differs(root: Path, files: list[str], run=subprocess.run) -> set[str]:
    """The `.rs` files of `files` that `tools/fmt.sh --stdin` would change: --check on only the PR's own files, ~1 s where the whole tree takes ~13."""

    def one(path: str) -> str | None:
        text = (root / path).read_text()
        done = run(["tools/fmt.sh", "--stdin", path], cwd=root, input=text, capture_output=True, text=True, errors="replace")
        return path if done.returncode or done.stdout != text else None

    rust = [f for f in files if f.endswith(".rs") and (root / f).is_file()]
    with ThreadPoolExecutor(max_workers=8) as pool:
        return {path for path in pool.map(one, rust) if path}


def precheck_checks(dev: Path, target: Path):
    """The cheap static checks of the gate, each `(name, root -> {check: the set of items failing})`: fmt's files, the docs' stale pages. Run on the
    dev checkout against the persistent dev target; the items are what a head's own failures are told apart from a PR's by."""
    env = {**os.environ, "CARGO_TARGET_DIR": str(target)}

    def fmt(root: Path, only: list[str] | None = None) -> dict[str, set[str]]:
        if only is not None:  # the PR side: its own files only (the baseline, computed once per feat/parity sha, checks them all)
            files = fmt_differs(root, only)
            return {"fmt": files} if files else {}
        done = subprocess.run(["tools/fmt.sh", "--check"], cwd=root, env=env, capture_output=True, text=True, errors="replace")
        files = {line.strip() for line in done.stdout.splitlines() if re.fullmatch(r"\S+\.(rs|py|toml)", line.strip())}
        return {"fmt": files or {"fmt --check failed"}} if done.returncode else {}

    def docs(root: Path) -> dict[str, set[str]]:
        done = subprocess.run([sys.executable, "tools/docs/docsbuild/build.py", "--check"], cwd=root, env=env, capture_output=True, text=True, errors="replace")
        if not done.returncode:
            return {}
        out = (done.stdout + done.stderr).splitlines()
        return {"docs": {line.strip() for line in out if line.strip().startswith("docs/")} or {"docsbuild --check failed"}}

    fmt.takes_only = True
    # no ratchets here: the batch's smoke runs all of them on the merged tree and on the head, so a red one is refused there (seconds to minutes per PR, median 80 s at load 70, were spent twice)
    return [("fmt", fmt), ("docs", docs)]


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
    info = info or json.loads(gh_run(["gh", "pr", "view", number, "--json", "headRefOid,title,body,baseRefName"], capture_output=True, text=True, errors="replace", check=True).stdout)
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
    # fmt is never excused by the head's own failures: every landing is formatted (the user's order), a red head file is formatted by the PR that touches it.
    new = {name: items - (set() if name == "fmt" else before.get(name, set())) for name, items in after.items() if name not in (head_failed or set())}
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
    done = gh_run(["gh", "pr", "view", number, "--json", "body", "--jq", ".body"], capture_output=True, text=True, errors="replace")
    return done.stdout if done.returncode == 0 else ""


BINARIES = ("llrm-cc", "llrm-llc", "llrm-qb", "llrm-nib")


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
    raise SystemExit(f"PARITY_BIN is unset and {found} lacks {', '.join(n for n in BINARIES if not (found / n).is_file())}: `cargo build --release --bins`, or set PARITY_BIN to a directory that has them")


def bins_command(directory: Path) -> list[str]:
    """The cargo build that refreshes every binary `directory` holds: `--bins` skips llrm-c (required-features = c-legacy), so a leftover llrm-c stayed older than the sources for good and the 03:12 pass was refused for it."""
    return ["cargo", "build", "--release", "--bins", "-q", *(["--features", "wccq"] if (directory / "llrm-c").is_file() else [])]


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
    built = (build or (lambda: subprocess.run(slotted(bins_command(directory)), cwd=cwd, env={**os.environ, **(env or {})}, capture_output=True, text=True, errors="replace")))()
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
            # the holder's pid, for the stage sampler and the runner's restart guards: they wait on this pid, not on a `pgrep -f` pattern
            with pidfile.recorded(f"{path}.pid"):
                yield
        finally:
            fcntl.flock(lock, fcntl.LOCK_UN)


GENERATED = ("docs/optimizations/passes/", "docs/optimizations/analyses/", "docs/optimizations/backend/", "docs/architecture/readme.md")  # what tools/docs/docsbuild/build.py --sync writes


def docs_stale_lines(output: str) -> list[str]:
    """The generated pages `build.py --check` names as out of date (its `  docs/...` lines), not the lint problems it also counts."""
    return [line.strip() for line in output.splitlines() if line.strip().startswith("docs/")]


def regenerate(conflicts: list[str], cwd: Path = ROOT) -> str:
    """"" when every conflicted path is generated and `build.py --sync --force` on the merged tree made it current (then `--check` agrees) and the merge was committed;
    else why not. Two PRs that each add a page both rewrite the index: a hand merge of generated text is what blocked the decide flip, the LSR stack and indvars."""
    if not conflicts or not all(path.startswith(GENERATED) for path in conflicts):
        return "conflicts in files nothing generates: " + ", ".join(path for path in conflicts if not path.startswith(GENERATED))
    for path in conflicts:
        git("checkout", "--ours", "--", path, cwd=cwd)
    build = [sys.executable, "tools/docs/docsbuild/build.py"]
    for flags in (["--sync", "--force"], ["--check"]):
        done = subprocess.run([*build, *flags], cwd=cwd, capture_output=True, text=True, errors="replace")
        if flags == ["--check"]:
            # the lint hits (`path: no # Example`) are the sources', not the pages': only a page still out of date after the sync is a failed regeneration (#4993 was refused for 4 baseline lint hits)
            if stale := docs_stale_lines(done.stdout + done.stderr):
                return f"docsbuild --check failed on the merged tree: pages still stale: {', '.join(stale[:3])}"
        elif done.returncode:
            return f"docsbuild {' '.join(flags)} failed on the merged tree: {(done.stdout + done.stderr).strip().splitlines()[-1:] or ['no output']}"
    git("add", "-A", "docs", cwd=cwd)
    done = subprocess.run(["git", "-c", "user.name=parity_squash", "-c", "user.email=parity_squash@localhost", "commit", "-q", "--no-edit"], cwd=cwd, capture_output=True, text=True, errors="replace")
    return "" if done.returncode == 0 else f"committing the regenerated merge failed: {done.stderr.strip()[:200]}"


def update_generated_only(head: str, branch: str, base: str = BASE, cwd: Path = ROOT) -> str:
    """The sha of `head` with `base` merged into it, pushed to `branch` as a fast-forward, when the only conflicts are generated pages (regenerated); "" when not.
    `gh pr merge` refuses a PR GitHub calls conflicting, so the local regeneration alone never landed it: six index-only PRs sat out the 03:02 batch."""
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp) / "w"
        git("worktree", "add", "-q", "--detach", str(work), head, cwd=cwd)
        try:
            merged = subprocess.run(["git", "-c", "user.name=parity_squash", "-c", "user.email=parity_squash@localhost", "merge", "--no-edit", "-q", base], cwd=work, capture_output=True, text=True, errors="replace")
            if merged.returncode and regenerate(git("diff", "--name-only", "--diff-filter=U", cwd=work).split(), work):
                return ""
            sha = git("rev-parse", "HEAD", cwd=work).strip()
            pushed = subprocess.run(["git", "push", "-q", "origin", f"{sha}:refs/heads/{branch}"], cwd=work, capture_output=True, text=True, errors="replace")
            return sha if pushed.returncode == 0 else ""
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", str(work)], cwd=cwd, capture_output=True)


def merge_parity(head: str, branch: str, base: str = BASE, cwd: Path = ROOT) -> str:
    """The sha of `head` with fresh `base` merged into it (a merge commit, no rebase), pushed to `branch` as a fast-forward; "" when the merge conflicts or the push fails. The
    owner's conflict stays the owner's: nothing is resolved here."""
    git("fetch", "-q", "origin", "feat/parity", cwd=cwd)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp) / "w"
        git("worktree", "add", "-q", "--detach", str(work), head, cwd=cwd)
        try:
            merged = subprocess.run(["git", "-c", "user.name=parity_squash", "-c", "user.email=parity_squash@localhost", "merge", "--no-edit", "-q", base], cwd=work, capture_output=True, text=True, errors="replace")
            if merged.returncode:
                return ""
            sha = git("rev-parse", "HEAD", cwd=work).strip()
            pushed = subprocess.run(["git", "push", "-q", "origin", f"{sha}:refs/heads/{branch}"], cwd=work, capture_output=True, text=True, errors="replace")
            return sha if pushed.returncode == 0 else ""
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", str(work)], cwd=cwd, capture_output=True)


def format_head(head: str, branch: str, base: str = BASE, cwd: Path = ROOT) -> tuple[str, str]:
    """(sha, ""): `head` with tools/fmt.sh applied to the .rs files its PR changes, the result committed on top and pushed to `branch` as a fast-forward; `head` itself when it is already
    formatted. (sha, why) with sha empty when fmt.sh fails with exit 2 or more (a missing toolchain, a crash: the landing is refused) or the push fails. Every landing is formatted."""
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp) / "w"
        git("worktree", "add", "-q", "--detach", str(work), head, cwd=cwd)
        try:
            files = [f for f in git("diff", "--name-only", f"{base}...{head}", cwd=work).split() if f.endswith(".rs") and (work / f).is_file()]
            changed = []
            for path in files:
                text = (work / path).read_text()
                done = subprocess.run(["tools/fmt.sh", "--stdin", path], cwd=work, input=text, capture_output=True, text=True, errors="replace")
                if done.returncode >= 2:
                    return "", f"tools/fmt.sh failed on {path} (exit {done.returncode}): {(done.stderr or done.stdout).strip()[-160:]}"
                if done.stdout and done.stdout != text:
                    (work / path).write_text(done.stdout)
                    changed.append(path)
            if not changed:
                return head, ""
            identity = ["-c", "user.name=parity_squash", "-c", "user.email=parity_squash@localhost"]
            subprocess.run(["git", *identity, "commit", "-q", "-am", "style: tools/fmt.sh on the files the PR changes"], cwd=work, check=True, capture_output=True)
            sha = git("rev-parse", "HEAD", cwd=work).strip()
            pushed = subprocess.run(["git", "push", "-q", "origin", f"{sha}:refs/heads/{branch}"], cwd=work, capture_output=True, text=True, errors="replace")
            return (sha, "") if pushed.returncode == 0 else ("", f"pushing the formatted head to {branch} failed: {pushed.stderr.strip()[-160:]}")
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", str(work)], cwd=cwd, capture_output=True)


def error_files(text: str) -> set[str]:
    """The `.rs` paths a build error names (`crates/x/src/y.rs:816:16`)."""
    return set(re.findall(r"[\w./-]+\.rs(?=:\d)", text))


def integration_probe(failed_units: set[str], red_on_base: set[str]) -> tuple[set[str], list[str]]:
    """(the `integration <target>::<test>` names the batch failed and the base passes, their targets). The names stay in the form `integration_failures` returns: stripped of
    `integration `, `probe & integration_failures(...)` was always empty, no PR was ever red alone or with its half, and the 16:17 batch cleared #5784 (target_facts) in 37 s."""
    probe = {name for name in failed_units if name.startswith("integration ")} - red_on_base
    return probe, sorted({name[len("integration ") :].split("::", 1)[0] for name in probe})


def build_offenders(group: list[str], merge_alone, check, errors: dict[str, str] | None = None, suspects=None) -> list[str]:
    """The PRs of `group` whose own merge on the head does not build: `merge_alone(number)` merges one onto a clean head (False when it cannot), `check()` is the build error or "". A batch that
    fails to build with no PR at fault alone (two halves that compile apart) names none and is halved. The 21:02 batch (#3763's new ISD variants against #3806's) refused every PR.
    `errors` receives each culprit's own build error: the 16:56 batch printed the batch's error for #4743, which built clean when asked alone.
    `suspects` are the PRs whose diff touches a file the batch's error names: only they are built alone first, and when one is at fault the rest are not (8 alone-builds of 50 s
    each for the 00:49 batch's one culprit); none at fault, every other PR is built alone as before."""
    found = []
    first = [number for number in group if number in (suspects or ())]
    for number in first + [number for number in group if number not in first]:
        if number not in first and any(one in found for one in first):
            break
        if merge_alone(number) and (error := check()):
            found.append(number)
            if errors is not None:
                errors[number] = error
    return [number for number in group if number in found]


def needs(package: str, graph: dict[str, set[str]]) -> set[str]:
    """`package` and every workspace package it depends on, through any chain: a PR can turn one of its tests red only by changing one of these."""
    out, todo = {package}, [package]
    while todo:
        for dep in graph.get(todo.pop(), ()):
            if dep not in out:
                out.add(dep)
                todo.append(dep)
    return out


def can_affect(files: list[str], package: str, packages: dict, graph: dict[str, set[str]], root: Path = ROOT) -> bool:
    """Whether a PR with `files` can change a test of `package`: it changes a package `package` needs, or a file no package owns (Cargo.toml, tools/, the root's tests/) which always probes."""
    smoke = parity_smoke_module()
    owned = smoke.touched_packages(files, packages, root)
    inside = [f for f in files if smoke.touched_packages([f], packages, root)]
    return len(inside) < len(files) or bool(owned & needs(package, graph))


def unit_offenders(group: list[str], by_package: dict[str, set[str]], merge_alone, failing, base_red=None, affects=None, own=None, reasons=None, why=None) -> list[str]:
    """The PRs of `group` whose merge alone fails a unit test of `by_package`. A test the base fails too (`base_red(by_package)` -> {(package, test)}) is no PR's: every PR merged
    on a red base fails it alone, and batch 2 refused or probed all 16. A PR that `affects(number, package)` says cannot change the package is not probed (a merge and a cargo run each).
    `own[number]` gets the `unit package::test` names that failed on that PR's probe (what `failing` returns; every probed test if it only says True), `reasons[number]` the `why(names)` read right
    after it: the 10:24 refusals of five PRs all carried the batch's whole list, so five different failures read as one."""
    red = base_red(by_package) if base_red else set()
    probe = {package: tests - {test for pkg, test in red if pkg == package} for package, tests in by_package.items()}
    probe = {package: tests for package, tests in probe.items() if tests}
    if not probe:
        return []
    found = []
    for number in group:
        mine = {package: tests for package, tests in probe.items() if affects is None or affects(number, package)}
        if not (mine and merge_alone(number)):
            continue
        for package, tests in mine.items():
            got = failing(package, tests)
            if got:
                found.append(number)
                names = sorted(f"unit {package}::{test}" for test in (tests if got is True else got))
                if own is not None:
                    own[number] = names
                if reasons is not None and why is not None:
                    reasons[number] = why(names)
                break
    return found


def refusal_line(number: str, named: dict[str, str], failed_build: list[str], failed_units: set[str]) -> str:
    """`#N: REFUSED: why` for a PR the batch dropped: the reason `named` holds (it failed what the batch failed alone), else the failure of the batch of one. The queue quoted the first REFUSED
    line of the shared log for every PR: at 22:26 eight PRs read as refused for #3841's `mut` while seven of them failed units."""
    why = named.get(number) or (failed_build[-1][:200] if failed_build else ", ".join(sorted(failed_units))[:200]) or "the batch of one does not pass"
    return f"#{number}: REFUSED: {why}"


HELD_STATE = Path(os.environ.get("LAND_TMP") or "/tmp/landing") / "held.state"  # `<PR> <head sha>` per line: the queue skips a PR at a head that was refused for itself


GITHUB_TRANSIENT = ("Something went wrong", "502 Bad Gateway", "API rate limit exceeded", "503 Service", "504 Gateway", "timed out")  # GitHub's side, not the PR's: #5039, 2026-10-27 00:14


def own_refusal(reason: str) -> bool:
    """Whether a refusal is the PR's own (its build, a test it fails alone on the head, its docs or format), which the same head will meet again, and not the batch's or the
    head's (a conflict, a stale merge base, GitHub's "head modified", a rescan after another PR was dropped), which a retry may clear."""
    return bool(reason) and not any(
        one in reason for one in ("conflicts with", "does not contain", "Head branch was modified", "Head branch is out of date", "does not merge into it cleanly", "not mergeable", "head moved", "Something went wrong", "API rate limit exceeded", "the batch dropped", "waits.txt")
    )


def hold_own_refusals(refused: list[str], reasons: dict[str, str], heads: dict[str, str], path: Path | None = None) -> list[str]:
    """Records `<PR> <head>` for each refused PR whose reason is its own, and returns the log lines: a PR refused for its own head cost a batch (1-14 min) every cycle with the
    same head (#4142 six times, #4145 and #4146 four, 21:47-22:22 on 2026-10-23); it re-enters when its head changes."""
    path = path or HELD_STATE
    lines = []
    for number in refused:
        if own_refusal(reasons.get(number, "")) and heads.get(number):
            path.parent.mkdir(parents=True, exist_ok=True)
            known = path.read_text().splitlines() if path.exists() else []
            kept = [one for one in known if one.split()[:1] != [number]]
            path.write_text("\n".join([*kept, f"{number} {heads[number]}"]) + "\n")
            lines.append(f"#{number}: held until head changes: {heads[number]}")
    return lines


def held_at(number: str, head: str, path: Path | None = None) -> bool:
    """Whether PR `number` at `head` was held."""
    path = path or HELD_STATE
    return path.exists() and f"{number} {head}" in path.read_text().splitlines()


def waiting(number: str, path: str | None = None) -> bool:
    """Whether `number` is in waits.txt (an owner's hold) now: read from the file at every call, not once at the batch's start. The queue is built from it before a batch, and a PR
    held while the batch ran landed anyway (#4057, 2026-10-23). `WAITS` is the live file of the queue runner; the batch's checkout has the committed one."""
    live = Path(path or os.environ.get("WAITS") or ROOT / "tools/landing/waits.txt")
    try:
        text = live.read_text()
    except OSError:
        return False
    return number in re.sub(r"#.*", "", text).split()


def conflict_detail(head: str, body: str, base: str = BASE, cwd: Path = ROOT) -> str:
    """` [owner W; conflicts in: file file]` for a PR whose squash GitHub refused for merge conflicts: the files `git merge-tree` finds when `head` meets `base`, and the `Owner:` line of its
    description. #5456 and #5462 were refused at squash with neither, and the coordinator probed each by hand."""
    done = subprocess.run(["git", "merge-tree", "--write-tree", "--name-only", "--no-messages", base, head], cwd=cwd, capture_output=True, text=True, errors="replace")
    files = done.stdout.splitlines()[1:] if done.returncode == 1 else []
    owner = (re.search(r"^\s*Owner:\s*(\S+)", body or "", re.M) or [None, "unknown"])[1]
    return f" [owner {owner}; conflicts in: {' '.join(files[:6]) or 'none found'}]"


def squash(number: str, info: dict, body: str, run=subprocess.run, update=update_generated_only, settle: float = 5, merge=merge_parity, retry_wait: float = 60) -> bool | None:
    """Squash PR `number` into feat/parity; None, with nothing pushed, when waits.txt holds it now. In fast mode a PR GitHub calls conflicting (only generated pages, regenerated) is first brought up to date on its branch, and says so in a comment."""
    run = rate_limit_proof(run)
    if waiting(number):
        print(f"#{number}: HELD: in waits.txt now; not pushed")
        return None
    if FAST and info["mergeable"] == "CONFLICTING":
        if not (fresh := update(info["headRefOid"], info["headRefName"])):
            return False
        note = f"The landing runner merged feat/parity into this branch ({fresh[:9]}; generated pages regenerated). If your push is rejected: git pull --no-rebase, then push; never force."
        run(["gh", "pr", "comment", number, "--body", note])
        info = {**info, "headRefOid": fresh}
        time.sleep(settle)  # GitHub recomputes the PR's mergeability after the push
    stage(f"squash #{number}")
    done = run(["gh", "pr", "merge", number, "--squash", "--match-head-commit", info["headRefOid"], "--subject", info["title"], "--body", body], capture_output=True, text=True, errors="replace")
    if done.returncode and "out of date" in f"{done.stderr}{done.stdout}".lower() and info.get("headRefName"):
        # Another landing moved feat/parity between the batch and this squash: the owner did nothing. Fresh parity is merged into the head once (a merge commit, no
        # force-push), only when it is conflict-free, and the squash retried once; a second refusal, or a conflict, is the owner's.
        if fresh := merge(info["headRefOid"], info["headRefName"]):
            print(f"#{number}: head out of date: merged parity {fresh[:9]}, retried")
            time.sleep(settle)  # GitHub takes the pushed head as the PR's before a --match-head-commit squash accepts it
            done = run(["gh", "pr", "merge", number, "--squash", "--match-head-commit", fresh, "--subject", info["title"], "--body", body], capture_output=True, text=True, errors="replace")
        else:
            print(f"#{number}: head out of date and feat/parity does not merge into it cleanly: the owner merges fresh feat/parity")
    if done.returncode and any(one in f"{done.stderr}{done.stdout}" for one in GITHUB_TRANSIENT):
        print(f"#{number}: GitHub failed on its side, retrying once after {retry_wait:g} s")
        time.sleep(retry_wait)
        done = run(["gh", "pr", "merge", number, "--squash", "--match-head-commit", info["headRefOid"], "--subject", info["title"], "--body", body], capture_output=True, text=True, errors="replace")
    if done.returncode and "Head branch was modified" in f"{done.stderr}{done.stdout}":
        HEAD_MOVED.add(number)
        print(f"#{number}: LEFT OUT (head moved): pushed while the batch ran; it re-enters the queue with its new head, not refused")
        return False
    if done.returncode:
        # GitHub's own words ("Head branch was modified", "not mergeable"): a refusal that named none read `4108: REFUSED: #4108 `
        why = ' '.join((done.stderr or done.stdout or 'no message').split())[-200:]
        detail = conflict_detail(info["headRefOid"], info.get("body") or pr_body(number)) if "merge conflicts" in why else ""
        print(f"#{number}: REFUSED: the squash was refused by GitHub: {why}{detail}")
    return done.returncode == 0


HEAD_MOVED: set[str] = set()  # PRs whose squash GitHub declined because the head moved: left out, never refused


def batch_summary(landed: list[str], refused: list[str], held: list[str], moved: list[str]) -> str:
    """The `LANDED: ... REFUSED: ...` line the runner reads; PRs left out because their head moved are listed apart and are not refused."""
    line = f"LANDED: {' '.join('#' + n for n in landed) or '-'}  REFUSED: {' '.join('#' + n for n in refused) or '-'}"
    return line + (f"  HELD: {' '.join('#' + n for n in held)}" if held else "") + (f"  LEFT OUT (head moved): {' '.join('#' + n for n in moved)}" if moved else "")


def merged_result(head: str, base: str = BASE, cwd: Path = ROOT) -> tuple[str | None, str]:
    """(sha, "") of `head` with `base` merged into it, checked out detached in `cwd` and not pushed; (None, why) when `base` conflicts. When `head` already
    contains `base` the result is `head` itself. The squash lands on whatever feat/parity is at the call: the tests must run on this result, not on a head that
    compiled beside a base that has since moved (#2544 and #2564 each compiled; together the head did not build for three hours). A conflict only in generated
    files is resolved by regenerating them (`regenerate`)."""
    stage("merge")
    git("checkout", "-q", "--detach", head, cwd=cwd)
    if subprocess.run(["git", "merge-base", "--is-ancestor", base, head], cwd=cwd).returncode:
        merged = subprocess.run(["git", "merge", "--no-edit", "-q", base], cwd=cwd, capture_output=True, text=True, errors="replace")
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
        text=True, errors="replace",
    )
    return smoke.returncode == 0, (smoke.stdout.strip().splitlines() or [""])[-1]


FAST = os.environ.get("LAND_FAST") == "1"  # the user's call (2026-10-19): no smoke, no tests, no precheck; each PR is merged in order and squashed; only a build of the combined tree can halve a batch

OVERLAP = os.environ.get("LAND_OVERLAP") == "1"  # the link check and the behavioural programs (release binaries) run beside the lib tests (debug); same checks, same order of refusal


def beside(background, foreground):
    """`foreground()`'s result and `background()`'s, the second running on a thread meanwhile; both are finished on return, also when `foreground` raises, as the next pass reuses the checkout."""
    with ThreadPoolExecutor(max_workers=1) as pool:
        future = pool.submit(background)
        try:
            front = foreground()
        finally:
            back = future.exception()
        if back is not None:
            raise back
        return front, future.result()


OVERLAP_MAX_LOAD = float(os.environ.get("OVERLAP_MAX_LOAD") or 3 * (os.cpu_count() or 1))  # 1-minute load above this: the two builds would only fight over the CPUs; the host runs at ~2.4 x its cores with other sessions' builds
OVERLAP_MIN_FREE_GB = 8  # two cargo builds at once; oomd kills sessions when memory runs out
OVERLAP_OFF_SECONDS = 3600


def overlap_flag() -> Path:
    return Path(os.environ.get("LAND_TMP", "/tmp")) / "overlap-off"


def free_gb() -> float:
    """MemAvailable in GB; a host that does not say counts as plenty."""
    try:
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemAvailable:"):
                return int(line.split()[1]) / 1024 / 1024
    except (OSError, ValueError, IndexError):
        pass
    return float("inf")


def overlap_verdict(load=os.getloadavg, now=time.time, flag: Path | None = None, free=free_gb) -> str:
    """Why the workspace check and the scanner build run in a row, or "" when they run together: for an hour after an overlapped run took longer than its two halves in a row, at a load
    above OVERLAP_MAX_LOAD, or with under OVERLAP_MIN_FREE_GB of memory free."""
    flag = flag or overlap_flag()
    if flag.exists() and now() - flag.stat().st_mtime < OVERLAP_OFF_SECONDS:
        return "the last overlapped run was slower than its halves in a row"
    if (current := load()[0]) > OVERLAP_MAX_LOAD:
        return f"load {current:.0f} is above {OVERLAP_MAX_LOAD:.0f}"
    if (spare := free()) < OVERLAP_MIN_FREE_GB:
        return f"{spare:.0f} GB free is under {OVERLAP_MIN_FREE_GB}"
    return ""


def check_beside_scanners(load=os.getloadavg, now=time.time, flag: Path | None = None, free=free_gb) -> bool:
    return not overlap_verdict(load, now, flag, free)


def check_and_scan(check, scan) -> tuple[str, bool]:
    """(the build error or "", whether the scan passed), the two run together. `beside` returns (foreground, background): the first draft of this unpacked them the other way round, so
    every overlapped batch called a passing build broken (`True[:300]`, a TypeError) and a failing one lost its text."""
    took: dict[str, float] = {}
    begun = time.monotonic()
    scanned, broken = beside(timed(took, "check", check), timed(took, "scanners", scan))
    note_overlap(time.monotonic() - begun, took["check"] + took["scanners"])
    return broken, scanned


def note_overlap(wall: float, serial: float, flag: Path | None = None) -> None:
    """Prints the overlapped wall time against its halves' sum; an overlap slower than that turns itself off for an hour."""
    print(f"check beside scanners: {wall:.0f} s against {serial:.0f} s in a row")
    if wall > serial:
        (flag or overlap_flag()).write_text(f"{wall:.0f} > {serial:.0f}\n")


def timed(took: dict[str, float], name: str, call):
    def run():
        begun = time.monotonic()
        try:
            return call()
        finally:
            took[name] = time.monotonic() - begun

    return run


MAX_BATCH = int(os.environ.get("BATCH_CAP") or (16 if FAST else 8))  # PRs per smoke: with ~38 ready PRs queued, batches of four took 12-16 minutes each; a failing batch is halved, so the cost of a bad PR is log2(8) more smokes


def batch(prs: list, passes, land, refuse, offenders=None, pending=None) -> None:
    """Land `prs` (up to MAX_BATCH, ready, in order) with ONE test of the combined tree. `passes(prs)` merges them all into the base and runs the fast set; when it fails the
    set is halved (2+2, then singles): the passing half lands, then the other half is tested on top of what landed. `refuse(pr)` names a PR that fails alone. `offenders(prs)`,
    when given, names the PRs that fail the failing unit alone: they are refused and the rest are tested once. `pending` is the list `land` appends to when it does not push
    (PUSH_MODE): those PRs are not on the base yet, so a later half is tested on top of them; without that two halves that build apart land together and the push fails
    on the pair (#5784 x #5830, "missing seh", 15:57 and 16:11: every PR passed alone, the set was assembled again each pass)."""
    if not prs:
        return
    under = [pr for pr in (pending or ()) if pr not in prs]
    if passes([*under, *prs]):
        try:
            land.group = list(prs)  # the members that land together: a rescan after a drop covers them once
        except AttributeError:  # a builtin as `land` (tests) has no attributes
            pass
        for pr in prs:
            land(pr)
        return
    if len(prs) == 1:
        if under:
            print(f"interaction: #{prs[0]} fails on top of {' '.join('#' + pr for pr in under)}")
        refuse(prs[0])
        return
    # A unit the head passes fails on the merged tree: the PRs that fail it alone are dropped by name and the rest are tested once, instead of a smoke per half.
    if offenders and (named := [pr for pr in offenders(prs) if pr in prs]) and len(named) < len(prs):
        for pr in named:
            refuse(pr)
        batch([pr for pr in prs if pr not in named], passes, land, refuse, offenders, pending)
        return
    middle = len(prs) // 2
    batch(prs[:middle], passes, land, refuse, offenders, pending)
    batch(prs[middle:], passes, land, refuse, offenders, pending)


SCAN_CRATE = "llrm-mir"  # the crate whose lib tests scan the other crates' sources


def offender_detail(names, members=None, near: bool = False) -> str:
    """` [offending files: #N path path; ...]` for the files a failing scan names, each under the PRs of the batch whose diff touches it (`members()` -> {PR: files}): the target_facts refusal
    of sqb-5378 listed three files and no PR, and the owners were found by hand. `near`: a file no PR touches names the PRs that touch its directory (a link panic in greedy/spliteditor.rs
    was another file's change, #5486's, in the same directory)."""
    paths = list(dict.fromkeys(re.findall(r"(?<![\w./-])[\w][\w.-]*(?:/[\w.-]+)+\.(?:rs|py|toml|md|sh|json)\b", " ".join(names))))
    if not paths or not members:
        return ""
    touched = members()
    by_pr = {number: [one for one in paths if one in files] for number, files in touched.items()}
    parts = [f"#{number} {' '.join(found)}" for number, found in by_pr.items() if found]
    left = [one for one in paths if not any(one in found for found in by_pr.values())]
    if near:
        for one in list(left):
            folder = one.rsplit("/", 1)[0] + "/"
            if neighbours := [f"#{number}" for number, files in touched.items() if any(other.startswith(folder) for other in files)]:
                parts.append(f"{one}: same directory touched by {' '.join(neighbours)}")
                left.remove(one)
    parts += [f"no PR of the batch: {' '.join(left)}"] if left else []
    return f" [offending files: {'; '.join(parts)}]" if parts else ""


def docs_detail(names, root: Path = ROOT, base: str = BASE, stale=None, members=None) -> str:
    """` [stale pages: <path> (batch tree <blob>, head <blob>; source <file>, changed by #N #M); ...]` when `docs_in_sync` is among `names`: the refusal said which test failed and owners guessed
    which page (03:03 batch, four refusals). The blobs are the page as the batch tree has it and as the head has it; the source is the file named in the page's `<!-- src: ... -->` line, and
    `members()` -> {PR: files its diff touches} names the PRs that changed it."""
    if not any("test_no_generated_page_is_stale" in name for name in names):
        return ""

    def blob(*args: str) -> str:
        done = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True)
        return done.stdout.strip()[:9] if done.returncode == 0 and done.stdout.strip() else "absent"

    pages = sorted((stale or docs_stale)(root))
    touched = members() if members else {}
    shown = []
    for page in pages[:5]:
        text = (root / page).read_text(errors="replace") if (root / page).exists() else ""
        source = (re.search(r"<!-- src: (\S+)", text) or re.search(r"header of \[`([^`]+)`", text) or [None, ""])[1]
        owners = [f"#{number}" for number, files in touched.items() if source and any(one == source or one.startswith(source.rstrip("/") + "/") for one in files)]
        extra = f"; source {source}, changed by {' '.join(owners) or 'no PR of the batch'}" if source else ""
        shown.append(f"{page} (batch tree {blob('hash-object', page) if (root / page).exists() else 'absent'}, head {blob('rev-parse', '--verify', '-q', f'{base}:{page}')}{extra})")
    return f" [stale pages: {'; '.join(shown)}{'; ...' if len(pages) > 5 else ''}]" if shown else ""


def warn_xpass(names, report: Path | None = None) -> None:
    """A listed (xfail) test that now passes is a warning and a line in `report`, not a refusal: land tempo first, the owner removes the row next. 25 refusals and ~106 batch-minutes in 24 h
    (2026-10-27) were these."""
    names = sorted(name for name in names if name.startswith("xpass "))
    if not names:
        return
    report = report or Path(os.environ.get("LAND_TMP", "/tmp")) / "xpass-report.txt"
    seen = report.read_text().splitlines() if report.exists() else []
    for name in names:
        print(f"WARNING {name}: listed in xfail.toml but passes; its owner removes the row (not a refusal)")
    report.parent.mkdir(parents=True, exist_ok=True)
    with report.open("a") as out:
        out.writelines(f"{name}\n" for name in names if name not in seen)


def new_lib_failures(
    files: list[str],
    on_merged=None,
    on_head=None,
) -> set[str]:
    """The failing lib tests of the crates `files` change (not their dependants) on the merged tree that the head, checked out where `on_head` runs them again, passes: a red test in a
    changed crate. #3578 landed four of them because the fast path builds the tests (cargo check --all-targets) and runs none. `on_merged(packages)` -> failing `unit <pkg>::<test>`
    names; `on_head(by_package)` -> those of them that also fail on the head (exact names, only the failing ones are run there)."""
    smoke = parity_smoke_module()
    packages, graph = smoke.workspace_graph()
    # The changed crates and every crate that depends on one: llrm-c's lib tests were red on parity while PRs merged, as each changed only a dependency.
    touched = smoke.dependants(smoke.touched_packages(files, packages), graph)
    # llrm-mir's scan tests (`no_file_gains_*`) read every crate's sources: a change to any .rs under crates/ can turn one red,
    # in a crate whose own lib tests ran (extload.rs, booleans.rs and selectiondag/ did, with llrm-mir untouched: three reds landed).
    if touched and any(f.startswith("crates/") and f.endswith(".rs") for f in files):
        touched = touched | {SCAN_CRATE}
    smoke.LAST_LIB.clear()
    failed = (on_merged or smoke.dependants_lib_failures)(touched) if touched else set()
    print(lib_tests_line(touched, smoke.LAST_LIB))
    # `xpass unit ...`: a listed test that passes: a warning and a report line
    xpass = {name for name in failed if name.startswith("xpass ")}
    failed -= xpass
    warn_xpass(xpass)
    if not failed:
        return set()
    by_package: dict[str, set[str]] = {}
    for name in failed:
        package, test = name[len("unit "):].split("::", 1)
        by_package.setdefault(package, set()).add(test)
    also = (on_head or _failing_on_head)(by_package)
    return failed - also


# Scanners the landing runner runs on every batch, whatever LAND_FAST_TESTS says (the user's call of 2026-10-19 is no tests at landing): reading docs costs milliseconds. A runner
# without the switch ran no scanner; the live one has it, and still landed #4039 with docs_links red (see scanner_tests_line).
ALWAYS_SCANNERS = (
    "block_list_ratchet",
    "core_depends_on_no_target",
    "core_fixtures_are_for_core_tests",
    "descriptions_are_read",
    "docs_in_sync",
    "docs_links",
    "dosbox_progress",
    "no_kill_by_pattern",
    "no_local_paths",
    "no_partial_ecx_counts",
    "no_todo",
    "selectiondag_registry",
    "target_facts",
    "tracked_build_products",
)


BUILD_SCRIPT_INPUTS = ("crates/target/", "crates/backend/llrm-iselgen/", "crates/codegen/llrm-codegen/src/backend/peep/")


def scanners_for(files: list[str]) -> tuple[str, ...]:
    """The default scan, plus build_script_reruns (three `cargo build --release --bins`, ~67 s) when the diff touches a build.rs, a Cargo.toml/Cargo.lock or a file the selector generator reads;
    the full scan (LAND_FAST_TESTS=1, nightly) runs it always."""
    if any(f.endswith(("build.rs", "Cargo.toml", "Cargo.lock")) or f.startswith(BUILD_SCRIPT_INPUTS) for f in files):
        return (*ALWAYS_SCANNERS, "build_script_reruns")
    return ALWAYS_SCANNERS


# Tests that launch DOSBox bench programs for minutes each (29 min for one pytest id): the CI gate runs them, a landing batch does not (its cap is TOOLTEST_CAP, 1800 s).
# Ids are relative to tools/, pytest's rootdir.
GATE_ONLY_TESTS = ("dosbatch/test_qb32.py::test_bench_program_prints_what_it_should_on_both_targets",)


def tool_tests_for(files: list[str], root: Path = ROOT) -> list[str]:
    """The pytest files of the Python tools a diff touches: tools/x.py -> tools/test_x.py, and a changed test file itself; none for a diff outside tools/."""
    tests = set()
    for name in files:
        path = Path(name)
        if path.suffix != ".py" or path.parts[:1] != ("tools",):
            continue
        test = path if path.name.startswith("test_") else path.with_name(f"test_{path.name}")
        if (root / test).is_file():
            tests.add(str(test))
    return sorted(tests)


def tool_test_failures(files: list[str], root: Path = ROOT, run=subprocess.run, ensure=None) -> list[str]:
    """The last lines of a failing pytest over `tool_tests_for(files)`; none when they pass or no tool changed. test_assemble sat red on parity for hours behind a landing that ran Rust tests only."""
    tests = tool_tests_for(files, root)
    if not tests:
        return []
    if why := (ensure or ensure_bins)():  # the tools resolve the release binaries and refuse stale ones: the 02:32 batch died on "stale binaries" here, before the build stage ran
        return [why]
    stage("tool-tests")
    cap = float(os.environ.get("TOOLTEST_CAP") or 1800)  # runner.env: seconds; a test file that runs 10-20 min refused every batch at the old 600
    try:
        done = run(["uv", "run", "-q", "--project", "tools", "python", "-m", "pytest", "-q", "-x", "-p", "no:cacheprovider", *[f"--deselect={one}" for one in GATE_ONLY_TESTS], *tests], cwd=root, capture_output=True, text=True, errors="replace", timeout=cap)
    except subprocess.TimeoutExpired:  # an uncaught timeout killed the batch with no REFUSED line, and the PR came back every pass
        return [f"the tool tests ran past {cap:g} s"]
    if done.returncode == 0:
        return []
    lines = (done.stdout + done.stderr).strip().splitlines()
    return [*[line for line in lines if line.startswith("E  ")][:20], *lines[-6:]]  # the last lines are the summary; the error text sits above them


def landing_scanner_failures(
    files: list[str],
    tests: bool,
    on_merged=None,
    on_head=None,
) -> set[str]:
    """The new red scanners of a batch: all of them with the lib tests on (LAND_FAST_TESTS=1), else the always-on ones."""
    return new_scanner_failures(files, on_merged, on_head, only=None if tests else scanners_for(files))


def new_scanner_failures(
    files: list[str],
    on_merged=None,
    on_head=None,
    only: tuple[str, ...] | None = None,
    seen: Path | None = None,
    now=time.time,
    root: Path = ROOT,
) -> set[str]:
    """The failing `integration <target>::<test>` of the root scanners (tests that read sources and docs, SCANNERS) and of a test file the batch changes, on the merged tree, that the head
    passes. #3899 landed with `tests/selectiondag_registry.rs` red (SD403 had no row): the layer ran lib tests only, and the target was in no map. `on_merged(targets)` and
    `on_head(targets)` -> the failing names."""
    smoke = parity_smoke_module()
    own = own_test_targets(files, ROOT)
    targets = sorted((set(only) if only else set(smoke.SCANNERS) | own) - set(smoke.NIGHTLY_ONLY))
    failed = (on_merged or smoke.integration_failures)(targets)
    warn_xpass(failed)
    failed = {name for name in failed if not name.startswith("xpass ")}
    print(scanner_tests_line(targets, failed, git("rev-parse", "HEAD", cwd=root).strip()[:9]))
    if not failed:
        return set()
    head = (on_head or _scanners_failing_on_head)(targets)
    # a red the head has is excused for the PR, but named every cycle with its age; one older than HEAD_RED_HOURS (and it has no xfail row, or it would not be here) refuses
    stale = excused_head_failures(failed & head, seen or Path(os.environ.get("LAND_TMP", "/tmp")) / "head-failure-seen.json", now())
    return (failed - head) | stale


HEAD_RED_HOURS = float(os.environ.get("HEAD_RED_HOURS") or 2)  # runner.env / the runner's environment: a red head older than this refuses every batch


def excused_head_failures(excused: set[str], path: Path, now: float) -> set[str]:
    """Prints each name of `excused` on its own line with the hours it has been red on the head, and returns those red longer than HEAD_RED_HOURS. The first time a name is seen is kept in `path`;
    a name no longer red is forgotten. #4993's docs_in_sync was red on the tip for hours behind a bare `1 red`."""
    if not excused and not path.exists():
        return set()
    first = json.loads(path.read_text()) if path.exists() else {}
    first = {name: first.get(name, now) for name in excused}
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(first))
    stale = set()
    for name in sorted(excused):
        hours = (now - first[name]) / 3600
        print(f"excused head failure: {name} (red on the head for {hours:.1f} h; no xfail row)")
        if hours > HEAD_RED_HOURS:
            stale.add(name)
    return stale


def new_link_failures(
    files: list[str],
    build=None,
    check=None,
    inputs=None,
) -> list[str]:
    """The programs that do not link on the merged tree, when the batch changes what programs link against (`link_check.touches_runtime`): a QB program and C programs built
    and linked, not run. qbrt went 183/218 on 50387f114 and the gate was green because nothing linked the QB runtime (a `libcalls` fact made its C sources call `memcpy_`)."""
    if os.environ.get("LAND_SKIP_LINK") == "1":  # the user's call while a break already on feat/parity is fixed forward: it refused every batch that touched the runtime
        print("link: skipped (LAND_SKIP_LINK=1)")
        return []
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import link_check  # noqa: PLC0415

    if bad := (inputs or link_check.unreadable_inputs)():
        print(f"link: {len(bad)} inputs unreadable, nothing built")
        return bad
    if not link_check.touches_runtime(files):
        print("link: skipped (nothing the programs link against changed)")
        return []
    (build or _build_release)()
    failures = (check or _link_programs)()
    print(f"link: {len(link_check.QB_PROGRAMS)} QB + {len(link_check.C_PROGRAMS)} C programs, {len(failures)} do not link")
    return failures


_BUILT: set[str] = set()


def alone_link_failures(files: list[str], cwd: Path, target: Path | None, run=subprocess.run) -> list[str]:
    """`tools/link_check.py` of the scratch tree `cwd` (its own release bins, built under `target/link`), when the PR touches the runtime or a frontend; the failing `qb <name>: why` / `c <name>: why` lines."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import link_check  # noqa: PLC0415
    from buildslot import slotted  # noqa: PLC0415

    if not (link_check.touches_runtime(files) or any(one.startswith("crates/frontends/") for one in files)):
        return []
    out = (target or cwd / "target") / "link"
    env = {**os.environ, "CARGO_TARGET_DIR": str(out)}
    built = run(slotted(["cargo", "build", "--release", "-q", "--bins"]), cwd=cwd, env=env, capture_output=True, text=True, errors="replace")
    if built.returncode:
        return [f"release: {(built.stdout + built.stderr).strip().splitlines()[-1:] or ['the bins do not build']}"]
    for name in ("llrm-cc", "llrm-llc", "llrm-qb", "llrm-nib", "nibfront"):
        (out / "release" / name).touch()  # the runners refuse a binary older than its sources
    done = run([sys.executable, "tools/link_check.py", str(out / "link-check")], cwd=cwd, env={**env, "LLRM_BIN": str(out / "release")}, capture_output=True, text=True, errors="replace")
    return [line for line in done.stdout.splitlines() if line.startswith(("qb ", "c "))]


def _build_release() -> None:
    """Release bins of the checkout, once per tree: the link layer and the behavioural gate share the build."""
    from buildslot import slotted  # noqa: PLC0415

    tree = git("rev-parse", "HEAD^{tree}").strip() + git("status", "--porcelain", "--untracked-files=no").strip()
    if tree in _BUILT:
        return
    release = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target") / "release"
    subprocess.run(slotted(bins_command(release)), cwd=ROOT, check=True)  # --bins alone leaves llrm-c as old as the last hand build, and the stages touch it fresh
    _BUILT.add(tree)


def pre_merge_behavioural(files: list[str], build=None, run=None) -> list[str]:
    """The call-shaped programs (tools/behavioural.py) that are wrong on the merged tree, when the batch changes backend source; none, and nothing built or run, otherwise.
    #4152 and #4146 each landed red from the backend and were found by the next hourly sweep, up to an hour later."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import behavioural  # noqa: PLC0415

    if not behavioural.touches(files):
        print("behavioural: skipped (no backend source changed)")
        return []
    started = time.time()
    (build or _build_release)()
    red = behavioural.red_programs((run or _run_behavioural)())
    if red:  # one run is no proof: the same code was 0 wrong at 14:44 and 23 wrong at 14:46 (LLRM_OW missing) and refused #5785
        print(f"behavioural: {len(red)} wrong; running once more on the same tree{red_env_cause()}")
        red = [program for program in behavioural.red_programs((run or _run_behavioural)()) if program in red]
    print(f"behavioural: {len(behavioural.PROGRAMS)} programs, {len(red)} wrong, {time.time() - started:.0f}s{red_env_cause() if red else ''}")
    return red


def pre_merge_stale(files: list[str], build=None, run=None) -> list[str]:
    """The finding lines of tools/oracle_sweep.py with `LLRM_CHECK_STALE` alone (every sweep program, -O2, both machines) on the merged tree's release bins, when the batch changes
    crate source; none, and nothing built or run, otherwise. The sweep ran at the gate only: 798dba289 landed and bintree, fpbench and jpeg_single failed it for ~2989 commits."""
    if not any(f.startswith("crates/") and f.endswith(".rs") and "_tests" not in f and "/tests/" not in f for f in files):
        print("stale: skipped (no crate source changed)")
        return []
    started = time.time()
    (build or _build_release)()
    done = (run or _run_stale)()
    print(f"stale: {done.stdout.strip().splitlines()[0] if done.stdout.strip() else 'no report'}, {time.time() - started:.0f}s")
    if done.returncode == 0:
        return []
    return [line.strip() for line in (done.stdout + done.stderr).splitlines() if line.strip() and not line.startswith("wall ")][:8] or [f"oracle_sweep exited {done.returncode} with no output"]


def stale_kinds(lines: list[str]) -> set[str]:
    """The finding kinds of a stale report, counts and example lists dropped: what a batch adds to a base that already fails."""
    return {line.strip().split(None, 1)[1] for line in lines if line.strip()[:1].isdigit() and " compiles / " not in line and len(line.split(None, 1)) == 2}


def _run_stale() -> subprocess.CompletedProcess:
    target = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target") / "release"
    command = [sys.executable, "tools/oracle_sweep.py", "--bin", str(target), "--checks", "STALE", "--levels=-O2", "--no-premise"]
    return subprocess.run(command, cwd=ROOT, capture_output=True, text=True, errors="replace", timeout=900)


def red_env_cause() -> str:
    """What a behavioural run needs from the environment and lacks, for the REFUSED line; empty when it has it."""
    ow = os.environ.get("LLRM_OW")
    return "" if ow and Path(ow).expanduser().is_dir() else " (LLRM_OW is unset or not a directory: a missing Open Watcom tree, not a miscompile)"


def _run_behavioural() -> str:
    import behavioural  # noqa: PLC0415

    target = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target") / "release"
    for one in target.iterdir():
        if one.is_file():
            one.touch()  # the runners refuse a binary older than its sources
    env = {**os.environ, "LLRM_BIN": str(target), "DOSBOX_BIN": str(target / "dosbox-x")}
    done = subprocess.run([sys.executable, "tools/dosbatch/run_tests.py", *behavioural.PROGRAMS], cwd=ROOT, env=env, capture_output=True, text=True, errors="replace", timeout=1800)
    return done.stdout + done.stderr


def head_files(head: str, base: str = BASE, cwd: Path = ROOT) -> list[str]:
    """The files `head` changes against `base`. A head pushed after the batch's fetch (the round re-reads heads) is not in the checkout: it is fetched first, where `git diff base...head` died with
    'Invalid symmetric difference' and took the whole batch down (11:04, #5183)."""
    if subprocess.run(["git", "cat-file", "-e", f"{head}^{{commit}}"], cwd=cwd, capture_output=True).returncode:
        git("fetch", "-q", "origin", head, cwd=cwd)
    return git("diff", "--name-only", f"{base}...{head}", cwd=cwd).split()


def halve_red(members: list[str], red_with) -> list[str]:
    """The members that are red on their own, found by halving: `red_with(subset)` merges the subset and asks. A 48-PR batch costs about 2 log2(48) runs per culprit, not 48."""
    if not members or not red_with(members):
        return []
    if len(members) == 1:
        return members
    mid = len(members) // 2
    return halve_red(members[:mid], red_with) + halve_red(members[mid:], red_with)


def red_members(members: list[str], red_with, alone_red) -> list[str]:
    """The red `members` by halving, else (an interaction: no half is red) each one merged alone and asked."""
    found = halve_red(members, red_with) if red_with else []
    return found or [number for number in members if alone_red(number)]


def files_named_in(text: str) -> set[str]:
    """The file paths a failure text names (`crates/x/src/y.rs: 7 copies of a target fact`, `tools/pairs/dyn.py: 1 copies`)."""
    return set(re.findall(r"[\w-]+(?:/[\w.-]+)+\.\w+", text))


def red_members_suspects_first(members: list[str], suspects: list[str], red_with, alone_red) -> list[str]:
    """`red_members`, asking the `suspects` (the PRs whose diff touches a file the failure names) first and the rest only when none of them is red: the 06:53 batch walked 37 PRs one by one,
    80 s each, for a copy in fpregs.rs that one PR touched."""
    suspects = [number for number in members if number in suspects]
    if not suspects or len(suspects) == len(members):
        return red_members(members, red_with, alone_red)
    return red_members(suspects, red_with, alone_red) or red_members([number for number in members if number not in suspects], red_with, alone_red)


def behavioural_culprits(group: list[str], files_of, alone_red, red_with=None) -> list[str]:
    """The PRs of `group` that turn the call-shaped programs red: the one backend PR of the batch when there is one (no build), else the backend PRs found by halving (`red_with`),
    else (an interaction: no half is red) each backend PR merged alone and asked."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import behavioural  # noqa: PLC0415

    backend = [number for number in group if behavioural.touches(files_of(number))]
    if len(backend) <= 1:  # a PR is blamed only after it is run alone: the batch's red may be the environment's
        return [number for number in backend if alone_red(number)]
    return red_members(backend, red_with, alone_red)


def _link_programs() -> list[str]:
    import link_check  # noqa: PLC0415

    target = Path(os.environ.get("CARGO_TARGET_DIR") or ROOT / "target")
    for name in ("llrm-cc", "llrm-llc", "llrm-qb", "llrm-nib", "nibfront"):
        (target / "release" / name).touch()  # the runners refuse a binary older than its sources
    os.environ["LLRM_BIN"] = str(target / "release")
    return link_check.link_failures(target / "link-check")


def rescan_alone(number: str, info: dict) -> set[str]:
    """The new red always-on scanners of PR `number` merged alone onto the head as it is now: the tree its squash makes after earlier PRs of the batch were dropped."""
    git("fetch", "-q", "origin", "feat/parity")
    clean_checkout(BASE)
    git("fetch", "-q", "origin", info["headRefOid"])
    if subprocess.run(["git", "merge", "--no-edit", "-q", info["headRefOid"]], cwd=ROOT, capture_output=True).returncode:
        subprocess.run(["git", "merge", "--abort"], cwd=ROOT, capture_output=True)
        return set()  # a conflict is the squash's to refuse
    files = git("diff", "--name-only", BASE, "HEAD").split()
    return new_scanner_failures(files, only=scanners_for(files))


def rescan_group_alone(numbers: list[str], infos: dict[str, dict]) -> set[str]:
    """The new red always-on scanners of the tree `numbers` make together on the head as it is now (the members still to land, merged one after another; a conflict is the squash's to refuse)."""
    git("fetch", "-q", "origin", "feat/parity")
    clean_checkout(BASE)
    for number in numbers:
        git("fetch", "-q", "origin", infos[number]["headRefOid"])
        if subprocess.run(["git", "merge", "--no-edit", "-q", infos[number]["headRefOid"]], cwd=ROOT, capture_output=True).returncode:
            subprocess.run(["git", "merge", "--abort"], cwd=ROOT, capture_output=True)
    subprocess.run([sys.executable, "tools/docs/docsbuild/build.py", "--sync", "--force"], cwd=ROOT, capture_output=True)  # each member's pages are current alone; together the pass counts of docs/optimizations/passes/readme.md are not (11:35-11:49: the group scan was red, each member green)
    files = git("diff", "--name-only", BASE, "HEAD").split()
    return new_scanner_failures(files, only=scanners_for(files))


def rescan_culprit(group: list[str], infos: dict[str, dict], rescan_group, rescan_one) -> tuple[str, set[str]] | None:
    """(member, its red scanners) when halving the red `group` ends on a member that is red alone, else None (an interaction): log2(n) group scans and one alone, where each member was rescanned in turn
    (~60-90 s each: 11:35-11:43 landed one PR a minute)."""
    pool = list(group)
    while len(pool) > 1:
        half = pool[: len(pool) // 2]
        pool = half if rescan_group(half, infos) else pool[len(pool) // 2 :]
    if pool and (failed := rescan_one(pool[0], infos[pool[0]])):
        return pool[0], failed
    return None


def make_rescan_check(rescan_group=rescan_group_alone, rescan_one=rescan_alone):
    """`check(number, infos, late_drops, group) -> why`: after the batch dropped a PR whose scan had passed, the remaining members' tree is scanned ONCE per new drop, not once per PR (~30 s each:
    10:48-10:50 squashes ran 35 s apart, 4 s of it the squash). Green: the rest land without a rescan until another PR is dropped. Red: the member that turns it red is found by halving and refused when
    its turn comes, the others land; when halving finds none (an interaction), each PR is rescanned alone, as before."""
    state = {"green": -1, "tried": -1, "culprit": None}

    def check(number: str, infos: dict[str, dict], late_drops: list[str], group: list[str]) -> str:
        if not late_drops:
            return ""
        if state["green"] == len(late_drops):
            return ""
        if state["tried"] != len(late_drops):
            state["tried"] = len(late_drops)
            state["culprit"] = None
            if not rescan_group(group, infos):
                state["green"] = len(late_drops)
                return ""
            state["culprit"] = rescan_culprit(group, infos, rescan_group, rescan_one)
        if state["culprit"]:
            member, failed = state["culprit"]
            if number != member:
                return ""
            return rescanned_refusal(number, infos[number], late_drops, rescan=lambda *_: failed)
        return rescanned_refusal(number, infos[number], late_drops, rescan=rescan_one)

    return check


def rescanned_refusal(number: str, info: dict, late_drops: list[str], rescan=rescan_alone) -> str:
    """Why PR `number` may not be pushed after the batch dropped `late_drops` (refused, held or failed at the squash) once its scan had passed: the tree it lands on was never scanned. The
    always-on scanners are run on it; "" when nothing was dropped or they pass."""
    if not late_drops:
        return ""
    failed = rescan(number, info)
    if not failed:
        return ""
    return f"the batch dropped {', '.join('#' + n for n in late_drops)} after its scan, and the tree #{number} lands on fails {', '.join(sorted(failed))}"


def scanner_tests_line(
    targets: list[str],
    failed: set[str],
    tree: str = "",
) -> str:
    """What the scanner layer did, printed on every batch it sees: a gate silent on a pass cannot be told from one that never ran. `tree` is the commit it scanned: #4039 landed an unlinked page
    after a batch line `0 red` that replaying the batch's PRs does not reproduce, and the log did not say which tree was scanned."""
    return f"scanners: {len(targets)} targets, {len(failed)} red" + (f" (tree {tree})" if tree else "")


def _scanners_failing_on_head(targets: list[str]) -> set[str]:
    """The scanners red on the head itself, once per head sha and target set: several are (docs naming a path, a link), and only a batch's new reds refuse it."""

    def compute() -> set[str]:
        clean_checkout(BASE)
        return parity_smoke_module().integration_failures(targets)

    key = hashlib.sha1(",".join(targets).encode()).hexdigest()[:10]
    return head_failures(git("rev-parse", BASE).strip(), compute, cache=HEAD_FAILURES / "scanners" / key)


def lib_tests_line(
    touched,
    stats: dict,
) -> str:
    """What the lib-test layer did, printed on every batch it sees, passing or not: a gate silent on a pass cannot be told from one that never ran."""
    if not touched:
        return "lib tests: 0 crates, 0 tests passed (no crate changed)"
    return f"lib tests: {len(touched)} crates, {stats.get('passed', '?')} tests passed ({stats.get('names', '?')}), {stats.get('seconds', 0):.0f}s"


_BASE_RED: dict[tuple, set] = {}  # (base sha, the tests asked) -> those that fail on the base: one cargo run per base, however many rounds ask


def _base_red(by_package: dict[str, set[str]]) -> set[tuple[str, str]]:
    key = (git("rev-parse", BASE).strip(), tuple(sorted((package, tuple(sorted(tests))) for package, tests in by_package.items())))
    if key not in _BASE_RED:
        clean_checkout(BASE)
        _BASE_RED[key] = {(package, test) for package, tests in by_package.items() for test in failing_on_checkout(package, tests)}
    return _BASE_RED[key]


def _failing_on_head(by_package: dict[str, set[str]]) -> set[str]:
    clean_checkout(BASE)
    return {f"unit {package}::{test}" for package, tests in by_package.items() for test in failing_on_checkout(package, tests)}


PUSH_MODE = os.environ.get("PUSH_MODE") == "1"  # runner.env: land a verified batch as one fast-forward push of its merge tip, not one squash per PR


def format_tip(cwd: Path = ROOT) -> bool:
    """tools/fmt.sh and the docs sync on the batch tip, committed when they change anything (the squash path pushes the same to each PR's branch). True when a commit was made."""
    subprocess.run(["tools/fmt.sh"], cwd=cwd, capture_output=True)
    subprocess.run([sys.executable, "tools/docs/docsbuild/build.py", "--sync", "--force"], cwd=cwd, capture_output=True)
    git("add", "-A", "docs", cwd=cwd)  # a new pass's generated page is untracked: `commit -a` skipped it, --check passed on the file on disk, and rpoattrs, deadmi and calledvaluepropagation landed without their page
    if not git("status", "--porcelain", "--untracked-files=no", cwd=cwd).strip():
        return False
    git("commit", "-qam", "style: fmt and generated docs of the batch", cwd=cwd)
    return True


def verify_tip(cwd: Path = ROOT) -> str:
    """Why the tip fails the cheap checks (fmt, docs), or "" when it passes: they run on the exact tip that is pushed."""
    if subprocess.run(["tools/fmt.sh", "--check"], cwd=cwd, capture_output=True).returncode:
        return "fmt --check fails on the tip"
    if subprocess.run([sys.executable, "tools/docs/docsbuild/build.py", "--check"], cwd=cwd, capture_output=True).returncode:
        return "docs --check fails on the tip"
    return ""


def push_tip(heads: dict[str, str], cwd: Path = ROOT, base: str = BASE, ref: str = "feat/parity", prepare=format_tip, verify=verify_tip) -> tuple[str, str]:
    """(tip, ""): the verified PR `heads` merged in order onto `base` (merge commits, the tests-append driver), formatted, checked, and pushed to `ref` as a plain fast-forward (never forced).
    GitHub marks each PR whose head is in the tip merged. ("", why) when a head does not merge, the tip fails `verify`, or the push is refused: the caller squashes per PR then."""
    clean_checkout(base, cwd)
    for number, head in heads.items():
        git("fetch", "-q", "origin", head, cwd=cwd)
        if subprocess.run(["git", "merge", "--no-edit", "-q", head], cwd=cwd, capture_output=True).returncode:
            subprocess.run(["git", "merge", "--abort"], cwd=cwd, capture_output=True)
            return "", f"#{number} does not merge on the tip"
    prepare(cwd)
    if why := verify(cwd):
        return "", why
    tip = git("rev-parse", "HEAD", cwd=cwd).strip()
    done = subprocess.run(["git", "push", "origin", f"{tip}:refs/heads/{ref}"], cwd=cwd, capture_output=True, text=True, errors="replace")
    if done.returncode:
        return "", f"push refused: {(done.stderr or done.stdout).strip()[:200]}"
    return tip, ""


def clean_checkout(ref: str, cwd: Path = ROOT) -> None:
    """`ref` checked out with nothing left from earlier batches: tracked changes reset and UNTRACKED files removed. A regenerated docs page left untracked made every later PR that adds
    it fail its local merge ("would be overwritten"), which read as a conflict and left such PRs out for hours."""
    git("reset", "-q", "--hard", cwd=cwd)
    git("clean", "-fdq", cwd=cwd)
    git("checkout", "-q", "--detach", ref, cwd=cwd)


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


def _in_target(target: Path | None, kind: str = "test"):
    """`subprocess.run` with CARGO_TARGET_DIR set to `target/<kind>`; the plain one when there is no target."""
    if target is None:
        return subprocess.run
    return lambda command, **kw: subprocess.run(command, env={**os.environ, "CARGO_TARGET_DIR": str(target / kind)}, **kw)


ALONE_DEPENDANTS = {"llrm-qb", "llrm-c", "llrm-nib"}  # the dependants whose lib tests failed batches that passed alone (#4927); the rest of the closure is the batch's to run
ALONE_JOBS = "2"  # PRs checked at once: three took 6 of 8 cargo processes beside the batch's tests
SOURCE_SCANNING_PACKAGES = {"llrm-mir"}  # lib tests that scan other crates' sources (no_file_gains_*): upstream of the crates they judge, so a dependants run never reaches them


def own_test_targets(files: list[str], root: Path) -> set[str]:
    """The integration targets of the `tests/<t>.rs` among `files` that exist in `root`: a PR forked before a test file landed lists it as deleted, and `cargo test --test <t>` of a missing
    target fails the whole run (#4784 was dropped as integration-did-not-run for `fill_loop_at_os`)."""
    return {Path(f).stem for f in files if f.startswith("tests/") and f.endswith(".rs") and "/" not in f[len("tests/") :] and (root / f).exists()}


def alone_refusals(head: str, base: str = BASE, cwd: Path = ROOT, check=None, scanners=None, differs=None, target: Path | None = None, units=None, on_base=None, docs=None, docs_on_base=None, links=None) -> list[str]:
    """Why the PR at `head`, alone on `base` (a scratch checkout at `base` in `cwd`), fails the cheap checks a batch would fail on: the merge, the workspace build, the
    always-on scanners (fmt differences only warn: the squash formats every PR). The 15:34 batch spent 62% of its time on reruns of PRs that failed these."""
    smoke = parity_smoke_module()
    check = check or ((lambda root: smoke.workspace_check(root, target / "warn")) if target else None)
    if why := merged_refusals(head, base, check, cwd):
        return why
    files = git("diff", "--name-only", base, "HEAD", cwd=cwd).split()
    if unformatted := sorted((differs or fmt_differs)(cwd, files)):
        print(f"alone: fmt.sh would change {', '.join(unformatted[:5])} (the squash formats it; not a refusal)", file=sys.stderr)
    # the scanners read the scratch tree: `integration_failures` defaults to ROOT, so every PR was judged on the tree the runner stands in
    # every scanner, not the always-on two: they read other crates' sources and docs, so a PR in one crate turns them red from outside it (#4871 added id-keyed maps in llrm-transforms; batch 18 died on llrm-mir's ratchet)
    def default_scanners(changed: list[str]) -> set[str]:
        # the scanners, the PR's own test files, and the integration targets xfail.toml lists that the PR can reach: one it makes pass is an xpass the gate refuses (#4673 cost batches 19 and 20)
        import xfail  # noqa: PLC0415

        packages, _ = smoke.workspace_graph(cwd)
        listed = {one.split()[1].split("::")[0] for one in xfail.load_for(cwd) if one.startswith("integration ")}
        reach = set(smoke.integration_targets_for(changed, smoke.touched_packages(changed, packages, cwd), smoke.integration_map(cwd))) & listed
        own = own_test_targets(changed, cwd)
        only = (set(smoke.SCANNERS) | own | reach) - set(smoke.NIGHTLY_ONLY)
        merged: set[str] = set()

        def on_merged(targets: list[str]) -> set[str]:
            merged.update(smoke.integration_failures(targets, root=cwd, run=_in_target(target)))
            return merged

        return new_scanner_failures(changed, on_merged, only=tuple(sorted(only)), root=cwd)

    red = sorted((scanners or default_scanners)(files))
    if red:
        return [f"scanner red alone: {', '.join(red[:5])}{smoke.causes_of(red)}"]
    # the generated pages the PR leaves out of date, less what the base already has stale: #4972 landed with 411 stale pages because LAND_FAST skips the precheck and nothing else ran the docs check
    if stale := (docs or docs_stale)(cwd):
        new = sorted(stale - (docs_on_base or (lambda: _alone_base_docs(base, target)))())
        if new:
            return [f"generated docs out of date (tools/docs/docsbuild/build.py --sync): {', '.join(new[:5])}"]
    # the QB and C programs, built and linked: #4822 and #4839 (frontend changes) reached three batches and cost 19 refusals because only a batch member touching the runtime made the batch link
    if broken := (links or alone_link_failures)(files, cwd, target):
        return [f"programs do not link: {', '.join('link ' + line.split(':')[0] for line in broken[:5])}"]
    # the unit tests of the crates the PR changes, less what the base fails: round 3 died on llrm-nib::test_pipeline::... and no PR was blamed
    def lib_failures(changed: list[str]) -> set[str]:
        packages, graph = smoke.workspace_graph(cwd)
        touched = smoke.touched_packages(changed, packages, cwd)
        # the crates that depend on a touched one too: #4927 changed llrm-transforms, passed alone, and failed four llrm-qb tests in the batch
        return smoke.dependants_lib_failures((touched | smoke.dependants(touched, graph) & ALONE_DEPENDANTS) | SOURCE_SCANNING_PACKAGES, root=cwd, run=_in_target(target, "lib")) if touched else set()

    failed = (units or lib_failures)(files)
    warn_xpass(failed)
    failed = {name for name in failed if not name.startswith("xpass ")}
    if not failed:
        return []
    by_package: dict[str, set[str]] = {}
    for name in failed:
        package, test = name[len("unit ") :].split("::", 1)
        by_package.setdefault(package, set()).add(test)
    red_on_base = (on_base or (lambda wanted: _alone_base_red(wanted, base, target)))(by_package)
    new = sorted(f"unit {package}::{test}" for package, tests in by_package.items() for test in tests - {t for p, t in red_on_base if p == package})
    return [f"unit tests fail alone: {', '.join(new)}"] if new else []


HITS = [0]  # verdicts read from the cache by this process
UNCHECKED: list[str] = []  # the PRs `alone` kept because the cap came before their verdict: in the batch with no check at all


def alone_refusal_line(number: str, why: str, head: str, directory: Path, width: int = 300) -> str:
    """The runner-log line of a refusal: all of `why` when it fits in `width`, else its start, its length and the file that holds all of it (#4991's list of failing tests was cut mid-name)."""
    if len(why) <= width:
        return f"{number}: REFUSED: alone: {why}"
    directory.mkdir(parents=True, exist_ok=True)
    full = directory / f"{number}-{head[:9]}.txt"
    full.write_text(why + "\n")
    return f"{number}: REFUSED: alone: {why[:width - 80]} ... ({len(why)} chars, all in {full})"


def _current_head(number: str) -> str:
    git("fetch", "-q", "origin", f"refs/pull/{number}/head")
    return git("rev-parse", "FETCH_HEAD").strip()


def alone(numbers: list[str], heads: dict[str, str], base: str = BASE, refusals=None, cache: Path | None = None, jobs: int = 1, cap: float | None = None, current=None, base_sha: str = "", recheck=None) -> tuple[list[str], dict[str, str]]:
    """(the numbers that pass alone, {number: why} for those that fail). A pass is kept per head sha in `cache`, so a PR checked while the previous batch squashed is not
    checked again; a refusal per head sha AND `base_sha`: it names what the base lacked (a conflict, a red scanner the base now fixes), and #5092 was refused at 02:31 for two reds
    the head had, then again at 02:53 and 03:03 from the cache. Except for `recheck(head, base)` (seconds: the head merged with the base as it is now, the generated pages), a pass is not run again: it was made against an older base, and a page a landing since made stale only shows on the merge. The uncached PRs run `jobs` at a time; the ones not finished `cap` seconds in are kept, unfiltered (a check of 40 PRs one after another took hours)."""
    verdicts: dict[str, str] = {}
    waiting: queue.Queue[str] = queue.Queue()
    rechecked: set[str] = set()
    for number in numbers:
        passed, refused = (cache / heads[number], cache / f"{heads[number]}@{base_sha[:12]}") if cache else (None, None)
        if passed and passed.exists() and not passed.read_text():
            HITS[0] += 1
            verdicts[number] = ""
            if recheck:
                rechecked.add(number)
                waiting.put(number)
        elif refused and base_sha and refused.exists():
            HITS[0] += 1
            verdicts[number] = refused.read_text()
        else:
            waiting.put(number)
    slots: queue.Queue[int] = queue.Queue()
    for slot in range(max(1, jobs)):
        slots.put(slot)

    def worker() -> None:
        while True:
            try:
                number = waiting.get_nowait()
            except queue.Empty:
                return
            slot = slots.get()
            try:
                if number in rechecked:
                    why = recheck(heads[number], base)
                else:
                    why = "; ".join(refusals(heads[number], base) if refusals else _alone_in_scratch(heads[number], base, slot, number in urgent_numbers()))
            except BaseException as error:  # a thread drops SystemExit silently: #5008 was kept with no verdict, unchecked and counted as checked, and broke the 21:53 batch
                why = f"alone check crashed: {type(error).__name__}: {str(error)[-200:]}"
            finally:
                slots.put(slot)
            if cache and number not in rechecked:
                cache.mkdir(parents=True, exist_ok=True)
                (cache / (heads[number] if not why else f"{heads[number]}@{base_sha[:12]}")).write_text(why)
            verdicts[number] = why

    threads = [threading.Thread(target=worker, daemon=True) for _ in range(max(1, jobs))]
    for thread in threads:
        thread.start()
    deadline = None if cap is None else time.monotonic() + cap
    for thread in threads:
        thread.join(None if deadline is None else max(0.0, deadline - time.monotonic()))
    # a refusal of a head the PR has since moved off is of a PR that no longer exists (#4993 was dropped at 21:04 for a conflict its 21:02 head did not have): kept, and its new head is checked next pass
    for number in [one for one in numbers if verdicts.get(one) and current]:
        if (now := current(number)) and now != heads[number]:
            del verdicts[number]
    UNCHECKED[:] = [number for number in numbers if number not in verdicts]
    kept = [number for number in numbers if not verdicts.get(number)]  # unchecked at the cap: kept
    return kept, {number: verdicts[number] for number in numbers if verdicts.get(number)}


def docs_stale(root: Path) -> set[str]:
    """The generated pages `tools/docs/docsbuild/build.py --check` reports out of date in `root` (`docsbuild --check: <its problem line>` when it fails without naming one)."""
    if not (root / "tools/docs/docsbuild/build.py").is_file():
        return set()
    done = subprocess.run([sys.executable, "tools/docs/docsbuild/build.py", "--check"], cwd=root, capture_output=True, text=True, errors="replace")
    if not done.returncode:
        return set()
    lines = [line.strip() for line in (done.stdout + done.stderr).splitlines() if line.strip()]
    if pages := {line for line in lines if line.startswith("docs/")}:
        return pages
    # a failure that names no page (a header with no `# Example`) is named by its own line, not by "failed": the sync cannot fix it
    problems = [line for line in lines if not re.match(r"\d+ problem|generated docs are out of date", line)]
    return {f"docsbuild --check: {'; '.join(problems[:2])}" if problems else "docsbuild --check failed"}


def docs_recheck(head: str, base: str, cwd: Path, base_docs: set[str]) -> str:
    """"" or why: the pages stale once `head` is merged into the checkout of `base` in `cwd`, less `base_docs`. The merge and `build.py --check` take seconds; a verdict cached against
    an older base missed #5062, #5085 and #5088, which passed alone and made lsr_solve.md, divrempairs.rs and sinkcast.md fail once merged with what landed after."""
    sha, why = merged_result(head, base, cwd)
    if sha is None:
        return why
    new = sorted(docs_stale(cwd) - base_docs)
    return f"generated docs out of date (tools/docs/docsbuild/build.py --sync): {', '.join(new[:5])}" if new else ""


def _alone_docs_recheck(head: str, base: str) -> str:
    root = Path(os.environ.get("ALONE_TARGETS", Path.home() / ".cache/llrm"))
    slot, lock = _claim_slot(root, int(os.environ.get("ALONE_SLOTS", "6")))
    targets = root / f"alone-{slot}"
    try:
        _alone_checkout(targets / "w", base)
        return docs_recheck(head, base, targets / "w", _alone_base_docs(base, targets))
    finally:
        lock.close()


def crash_line(number: str, error: BaseException) -> str:
    """`#N: REFUSED: the batch crashed: <exception>`: a TypeError in docs_gated (05:18) left 13 PRs refused with an empty reason, the traceback on a line no summary carries."""
    return f"#{number}: REFUSED: the batch crashed (a tool fault, not this PR): {type(error).__name__}: {' '.join(str(error).split())[:200]}"


def docs_refusal(before: set[str], root: Path, docs=docs_stale) -> tuple[str, set[str]]:
    """(why, stale now): a refusal naming the pages `root` has out of date that `before` did not. #5031, #5049 and #4988 each changed a `//!` header, passed alone and landed
    with six pages stale: the head's red docs_in_sync was excused, so the batch never saw a new one."""
    now = docs(root)
    new = sorted(now - before)
    return (f"generated docs out of date (tools/docs/docsbuild/build.py --sync): {', '.join(new[:5])}" if new else ""), now


def docs_sync_commit(root: Path = ROOT) -> str:
    """"" when `tools/docs/docsbuild/build.py --sync` in `root` ran and its pages are committed on top (nothing to commit is fine); else why. Generated docs out of date refused #5129, #5131 and #5119 for a
    page the landing could have written, as it formats an unformatted head."""
    done = subprocess.run([sys.executable, "tools/docs/docsbuild/build.py", "--sync"], cwd=root, capture_output=True, text=True, errors="replace")
    if done.returncode:
        return f"docsbuild --sync failed: {' '.join((done.stdout + done.stderr).split()[-12:])}"
    git("add", "-A", "docs", cwd=root)
    if subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=root).returncode == 0:
        return ""
    made = subprocess.run(["git", "-c", "user.name=parity_squash", "-c", "user.email=parity_squash@localhost", "commit", "-q", "-m", "docs: tools/docs/docsbuild/build.py --sync"], cwd=root, capture_output=True, text=True, errors="replace")
    return "" if made.returncode == 0 else f"committing the synced pages failed: {made.stderr.strip()[:200]}"


def docs_gated(merge_one, docs_before: set[str], root: Path = ROOT, docs=docs_stale, sync=docs_sync_commit):
    """`merge_one` that, when the merge leaves a generated page stale that was not stale before it, syncs the pages and commits them on top (`.synced` names those PRs: the landing pushes the same
    sync to their branch); it refuses, and resets `root` to where it was, only when the sync fails or leaves a page stale."""
    state = [docs_before]
    synced: set[str] = set()

    def gated(number: str) -> str:
        before = git("rev-parse", "HEAD", cwd=root).strip()
        if why := merge_one(number):
            return why
        why, now = docs_refusal(state[0], root, docs)
        if why and not sync(root):
            why, now = docs_refusal(state[0], root, docs)
            if not why:
                synced.add(number)
        if why:
            git("reset", "-q", "--hard", before, cwd=root)
            return why
        state[0] = now
        return ""

    gated.synced = synced
    return gated


def sync_docs_head(head: str, branch: str, base: str = BASE, cwd: Path = ROOT) -> tuple[str, str]:
    """(sha, ""): `head` with `base` merged in, `build.py --sync` applied and committed, pushed to `branch` as a fast-forward, so the squash lands current pages; `head` itself when its pages are
    current after the merge. (\"\", why) when the merge conflicts outside generated pages, the sync fails or leaves a page stale, or the push fails."""
    git("fetch", "-q", "origin", "feat/parity", cwd=cwd)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp) / "w"
        git("worktree", "add", "-q", "--detach", str(work), head, cwd=cwd)
        try:
            merged = subprocess.run(["git", "-c", "user.name=parity_squash", "-c", "user.email=parity_squash@localhost", "merge", "--no-edit", "-q", base], cwd=work, capture_output=True, text=True, errors="replace")
            if merged.returncode and (why := regenerate(git("diff", "--name-only", "--diff-filter=U", cwd=work).split(), work)):
                return "", why
            if not docs_stale(work):
                return head, ""
            if why := docs_sync_commit(work):
                return "", why
            if still := docs_stale(work):
                return "", f"pages still stale after build.py --sync: {', '.join(sorted(still)[:3])}"
            sha = git("rev-parse", "HEAD", cwd=work).strip()
            pushed = subprocess.run(["git", "push", "-q", "origin", f"{sha}:refs/heads/{branch}"], cwd=work, capture_output=True, text=True, errors="replace")
            return (sha, "") if pushed.returncode == 0 else ("", f"pushing the synced head to {branch} failed: {pushed.stderr.strip()[-160:]}")
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", str(work)], cwd=cwd, capture_output=True)


def _alone_base_docs(base: str, target: Path | None) -> set[str]:
    work = (target or Path(tempfile.mkdtemp(prefix="alone-base-"))) / "base"
    _alone_checkout(work, base)
    return docs_stale(work)


def _alone_base_red(by_package: dict[str, set[str]], base: str, target: Path | None) -> set[tuple[str, str]]:
    """{(package, test)} of `by_package` that fail on `base` itself: a checkout of its own beside the slot's, so the batch's tree is never touched."""
    work = (target or Path(tempfile.mkdtemp(prefix="alone-base-"))) / "base"
    _alone_checkout(work, base)
    env = {**os.environ, "CARGO_TARGET_DIR": str((target or work.parent) / "lib")}
    return {(package, test) for package, tests in by_package.items() for test in failing_on_checkout(package, tests, cwd=work, env=env)}


def _alone_checkout(work: Path, ref: str, repo: Path = ROOT) -> None:
    """A worktree at a path that stays: cargo fingerprints by path, so a fresh directory per PR rebuilt everything (2-3 min a PR); reused, only the files that differ rebuild."""
    if (work / ".git").exists():
        clean_checkout(ref, cwd=work)
    else:
        work.parent.mkdir(parents=True, exist_ok=True)
        git("worktree", "add", "-q", "--detach", str(work), ref, cwd=repo)


def urgent_numbers(environ=os.environ) -> set[str]:
    """The PR numbers of `$LAND_TMP/urgent.txt` (numbers, `#` comments): landing fixes, which may use the reserved alone slot."""
    path = Path(environ.get("LAND_TMP", "/tmp")) / "urgent.txt"
    return set(re.findall(r"\b\d+\b", re.sub(r"#.*", "", path.read_text()))) if path.exists() else set()


def _claim_slot(root: Path, count: int, urgent: bool = False, reserved: int = 1):
    """(slot, lock file) of the first slot no other `alone` process holds; waits for one. Two processes (the filter and a warm-up) never share a checkout.
    The last `reserved` slots are for `urgent` PRs (landing fixes) only, which try them first: with every slot held by other checks a fix waited behind them."""
    first = count - reserved if urgent and count > reserved else 0
    usable = list(range(first, count)) + list(range(first)) if urgent else list(range(max(1, count - reserved)))
    while True:
        for slot in usable:
            root.mkdir(parents=True, exist_ok=True)
            handle = open(root / f"alone-{slot}.lock", "w")
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                return slot, handle
            except OSError:
                handle.close()
        time.sleep(2)


def keep_refused_evidence(head: str, cwd: Path, why: list[str], directory: Path) -> None:
    """For a generated-docs refusal, `HEAD`, `git status` and `build.py --check` of the tree that was judged, in `directory/<head>/`: the next PR's check resets the slot, and a refusal
    that the same tree does not reproduce (docs_in_sync at 01:41, 01:44, 02:08, 02:26) left no evidence of what was stale."""
    if not any("docs_in_sync" in one or "generated docs" in one for one in why):
        return
    out = directory / head[:12]
    out.mkdir(parents=True, exist_ok=True)
    (out / "head.txt").write_text(f"tree {git('rev-parse', 'HEAD', cwd=cwd).strip()}\npr head {head} committed {subprocess.run(['git', 'show', '-s', '--format=%cI', head], cwd=cwd, capture_output=True, text=True).stdout.strip() or '?'}\n")
    (out / "status.txt").write_text(git("status", "--short", cwd=cwd))
    check = subprocess.run([sys.executable, "tools/docs/docsbuild/build.py", "--check"], cwd=cwd, capture_output=True, text=True, errors="replace")
    (out / "check.txt").write_text(check.stdout + check.stderr)


def keep_batch_evidence(heads: dict[str, str], cwd: Path, directory: Path, stamp: str) -> None:
    """The batch tree's HEAD, status and `build.py --check`, and each member's head sha and commit time, in `directory/batch-<stamp>/`: "stale heads or the runner's tree" is answerable
    from it at the next docs refusal."""
    out = directory / f"batch-{stamp}"
    out.mkdir(parents=True, exist_ok=True)
    lines = [f"tree {git('rev-parse', 'HEAD', cwd=cwd).strip()}"]
    for number, head in heads.items():
        lines.append(f"#{number} {head} committed {subprocess.run(['git', 'show', '-s', '--format=%cI', head], cwd=cwd, capture_output=True, text=True).stdout.strip() or '?'}")
    (out / "heads.txt").write_text("\n".join(lines) + "\n")
    (out / "status.txt").write_text(git("status", "--short", cwd=cwd))
    check = subprocess.run([sys.executable, "tools/docs/docsbuild/build.py", "--check"], cwd=cwd, capture_output=True, text=True, errors="replace")
    (out / "check.txt").write_text(check.stdout + check.stderr)


def _alone_in_scratch(head: str, base: str, slot: int = 0, urgent: bool = False) -> list[str]:
    root = Path(os.environ.get("ALONE_TARGETS", Path.home() / ".cache/llrm"))
    slot, lock = _claim_slot(root, int(os.environ.get("ALONE_SLOTS", "6")), urgent, int(os.environ.get("ALONE_RESERVED", "1")))
    targets = root / f"alone-{slot}"  # one cargo target and one checkout per slot
    try:
        _alone_checkout(targets / "w", base)
        print(f"alone: slot {slot} base {git('rev-parse', '--short=9', 'HEAD', cwd=targets / 'w').strip()}", file=sys.stderr)
        why = alone_refusals(head, base, targets / "w", target=targets)
        keep_refused_evidence(head, targets / "w", why, Path(os.environ.get("LAND_TMP", "/tmp")) / "refused")
        return why
    finally:
        lock.close()


ADDED_ITEM = re.compile(r"^\+\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:fn|def)\s+(\w+)", re.M)


def added_names(head: str, base: str, cwd: Path) -> set[str]:
    """The function names a PR's head adds against `base` (the merge base): two PRs adding one test name merge without a conflict and fail to compile together
    ('defined multiple times': #4678 and #4680 both added a_frame_cell_with_a_wide_index_is_not_encodable_before_the_frame_is_named)."""
    diff = subprocess.run(["git", "diff", "-U0", f"{base}...{head}"], cwd=cwd, capture_output=True, text=True, errors="replace").stdout
    return set(ADDED_ITEM.findall(diff))


def needs_the_merge_driver(before: str, head: str, cwd: Path) -> list[str]:
    """The files `head` conflicts in with `before` under plain git (the tests-append driver off): where it is not empty, the batch's merge joined the PR only through that driver and
    GitHub's squash, which has none, will refuse it as conflicting (#5462, #5481, #5591 in one day)."""
    env = {**os.environ, "GIT_CONFIG_COUNT": "0"}
    done = subprocess.run(["git", "-c", "merge.tests-append.driver=false", "merge-tree", "--write-tree", "--name-only", "--no-messages", before, head], cwd=cwd, env=env, capture_output=True, text=True, errors="replace")
    return done.stdout.splitlines()[1:] if done.returncode == 1 else []


def assemble(numbers: list[str], heads: dict[str, str], base: str = BASE, cwd: Path = ROOT, limit: int = 16) -> tuple[list[str], dict[str, str]]:
    """The first `limit` of `numbers`, in order, that merge into `base` one after another without a conflict (`fast_merge`'s own merge, in a scratch worktree), and why
    each other waits. A batch of these loses no PR to a conflict with an earlier member: 63 of 93 conflict refusals in a week were with a member that landed, and the run
    that left them out was a full smoke for the others."""
    kept: list[str] = []
    waits: dict[str, str] = {}
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp) / "w"
        git("worktree", "add", "-q", "--detach", str(work), base, cwd=cwd)
        try:
            claimed: dict[str, str] = {}  # a name a kept PR adds -> that PR
            started = time.monotonic()

            def progress(number: str, outcome: str) -> None:
                """One line per candidate on stderr (the runner's assemble.log): a 13-minute assembly printed nothing and nobody knew how far it was."""
                print(f"assemble: #{number} {outcome} ({len(kept)}/{limit} kept, {time.monotonic() - started:.0f} s)", file=sys.stderr, flush=True)

            for number in numbers:
                if len(kept) >= limit:
                    break
                names = added_names(heads[number], base, cwd)
                if clash := sorted(name for name in names if name in claimed):
                    waits[number] = f"adds {clash[0]}, which #{claimed[clash[0]]} adds too: one of them drops it"
                    progress(number, "waits (same name)")
                    continue
                before = git("rev-parse", "HEAD", cwd=work).strip()
                merge = subprocess.run(["git", "-c", "user.name=parity_squash", "-c", "user.email=parity_squash@localhost", "merge", "--no-edit", "-q", heads[number]], cwd=work, capture_output=True, text=True, errors="replace")
                if not merge.returncode and (driven := needs_the_merge_driver(before, heads[number], work)):
                    git("reset", "-q", "--hard", before, cwd=work)
                    waits[number] = "needs owner parity merge: it conflicts without the tests-append driver in " + ", ".join(driven[:4])
                    continue
                if merge.returncode:
                    conflicts = git("diff", "--name-only", "--diff-filter=U", cwd=work).split()
                    if conflicts and not regenerate(conflicts, work):
                        kept.append(number)
                        claimed.update(dict.fromkeys(names, number))
                        progress(number, "kept (regenerated)")
                        continue
                    subprocess.run(["git", "merge", "--abort"], cwd=work, capture_output=True)
                    waits[number] = "conflicts with an earlier PR of the batch: " + ", ".join(conflicts or ["the tree"])
                    progress(number, "waits (conflict)")
                else:
                    kept.append(number)
                    claimed.update(dict.fromkeys(names, number))
                    progress(number, "kept")
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", str(work)], cwd=cwd, capture_output=True)
    return kept, waits


def batch_blame(heads: dict[str, str], base: str, cwd: Path = ROOT) -> list[str]:
    """One line per PR of a batch naming the markdown files it adds, deletes or moves: the doc a `docs_*` scanner finds unreachable or stale is one of them (#5008 added a page nothing linked
    and the refusal named only the test)."""
    lines = []
    for number, head in heads.items():
        out = subprocess.run(["git", "diff", "--name-status", f"{base}...{head}", "--", "*.md"], cwd=cwd, capture_output=True, text=True, errors="replace").stdout.splitlines()
        if out:
            lines.append(f"  #{number}: " + ", ".join(" ".join(line.split()) for line in out[:8]))
    return lines


GQL_PR = "state isDraft headRefOid headRefName title body baseRefName mergeable commits(last: 1) { nodes { commit { committedDate statusCheckRollup { state } } } }"


def graphql_prs(numbers: list[str], run=None) -> dict[str, dict]:
    """{number: state, isDraft, headRefOid, headRefName, title, body, baseRefName, mergeable, committedDate, checks} for every PR GitHub knows in `numbers`, from one `gh api graphql` call
    per 40 PRs. A `gh pr view` each cost a REST call per PR per stage: about 100 a cycle, and the quota ran out at 09:2x."""
    owner, name = os.environ.get("R", "ali-mosavian/llrm").split("/")
    found: dict[str, dict] = {}
    for start in range(0, len(numbers), 40):
        chunk = numbers[start : start + 40]
        body = " ".join(f"p{n}: pullRequest(number: {n}) {{ {GQL_PR} }}" for n in chunk)
        query = f'query {{ repository(owner: "{owner}", name: "{name}") {{ {body} }} }}'
        done = (run or gh_run)(["gh", "api", "graphql", "-f", f"query={query}"], capture_output=True, text=True, errors="replace", check=True)
        repository = json.loads(done.stdout)["data"]["repository"]
        for number in chunk:
            if pr := repository.get(f"p{number}"):
                commit = (pr.pop("commits")["nodes"] or [{"commit": {}}])[0]["commit"]
                found[number] = {**pr, "committedDate": commit.get("committedDate", ""), "checks": (commit.get("statusCheckRollup") or {}).get("state", "")}
    return found


PR_FIELDS = "headRefOid,headRefName,title,body,baseRefName,mergeable"


def pr_info(number: str) -> dict:
    return json.loads(gh_run(["gh", "pr", "view", number, "--json", PR_FIELDS], capture_output=True, text=True, errors="replace", check=True).stdout)


def refresh_heads(infos: dict[str, dict], numbers: list[str], view=None) -> dict[str, tuple[str, str]]:
    """{number: (old head, new head)} of the PRs whose head moved since `infos` was read, which `infos` now holds. A batch that halves and runs again for an hour kept merging the
    heads it read at its start: at 02:34 the sub-batch merged #5088's 01:18 head with sinkcast.md stale, 30 min after w12 pushed the fix, and the squash of #5085 was refused for a head
    its owner had moved."""
    moved = {}
    fetched = graphql_prs(numbers) if view is None else {}
    for number in numbers:
        if view is None and number not in fetched:
            continue
        now = fetched[number] if view is None else view(number)
        if now["headRefOid"] != infos[number]["headRefOid"]:
            moved[number] = (infos[number]["headRefOid"], now["headRefOid"])
            infos[number] = now
    return moved


BATCH_MAX_SECONDS = int(os.environ.get("BATCH_MAX_SECONDS", "1200"))  # a merge-batch older than this stops: its assembly (who is eligible, at which head) is no longer the queue's


class BatchTooOld(Exception):
    pass


def batch_expired(started: float, now: float, cap: float = BATCH_MAX_SECONDS) -> bool:
    return now - started > cap


def defer_moved(infos: dict[str, dict], group: list[str], left_out: dict[str, str], view=pr_info) -> dict[str, tuple[str, str]]:
    """Leaves out of the batch the PRs whose head moved since it was read: the new head has not waited its own head-age (a fix may be on its way), so it joins a later batch."""
    moved = refresh_heads(infos, group, view)
    for number, (old, new) in moved.items():
        left_out[number] = f"head moved {old[:9]} -> {new[:9]} since the batch began; the new head waits its own head age for a later batch"
    return moved


def merge_batch(numbers: list[str], known: dict[str, str] | None = None) -> int:
    """`parity_squash.py merge-batch N1..N8`: the PRs in order, one smoke on the base with all of them merged in, each squashed with its head lease."""
    known = known or {}
    if len(numbers) > MAX_BATCH:
        raise SystemExit(f"merge-batch takes at most {MAX_BATCH} PRs per smoke, got {len(numbers)}: split the queue")
    infos = {}
    known_prs = graphql_prs(numbers)
    for number in numbers:
        if number not in known_prs:
            raise SystemExit(f"#{number} is not a PR GitHub knows")
        info = known_prs[number]
        if info["baseRefName"] != "feat/parity":
            raise SystemExit(f"#{number} targets {info['baseRefName']}, not feat/parity")
        infos[number] = info
    landed, refused, held, late_drops, moved = [], [], [], [], []
    docs_synced: set[str] = set()  # PRs whose merge staled a generated page: synced in the batch tree, and on their branch before the squash
    started = time.monotonic()

    left_out: dict[str, str] = {}
    failed_units: set[str] = set()
    failed_build: list[str] = []
    named: dict[str, str] = {}

    def refuse(number: str) -> None:
        print(refusal_line(number, named, failed_build, failed_units))
        refused.append(number)

    def passes_fast(group: list[str]) -> bool:
        git("fetch", "-q", "origin", "feat/parity")
        clean_checkout(BASE)

        def members_files() -> dict[str, list[str]]:
            return {number: git("diff", "--name-only", f"{BASE}...{infos[number]['headRefOid']}").split() for number in group}

        def merge_one(number: str) -> str:
            if number in left_out:
                return left_out[number]
            before = git("rev-parse", "HEAD").strip()
            git("fetch", "-q", "origin", infos[number]["headRefOid"])
            merge = subprocess.run(["git", "merge", "--no-edit", "-q", infos[number]["headRefOid"]], cwd=ROOT, capture_output=True, text=True, errors="replace")
            if merge.returncode and (conflicts := git("diff", "--name-only", "--diff-filter=U").split()) and not regenerate(conflicts):
                merge = subprocess.CompletedProcess([], 0)  # only generated docs conflicted: regenerated and committed
            if merge.returncode:
                subprocess.run(["git", "merge", "--abort"], cwd=ROOT, capture_output=True)
                return "conflicts with an earlier PR of the batch or with feat/parity: its owner merges fresh feat/parity"
            for note in warnings(git("rev-parse", "HEAD").strip(), infos[number]["body"] or "", base=before, title=infos[number]["title"]):
                print(f"#{number}: WARNING: {note}")
            if why := refusals(git("rev-parse", "HEAD").strip(), infos[number]["body"] or "", base=before, title=infos[number]["title"]):
                git("reset", "-q", "--hard", before)
                return "; ".join(why)
            return ""

        failed_units.clear()
        failed_build.clear()
        stage("fast-merge")
        print(f"members {len(group)}: {' '.join('#' + number for number in group)}")
        gate = docs_gated(merge_one, docs_stale(ROOT))
        kept, out = fast_merge(group, gate)
        docs_synced.update(gate.synced & set(kept))
        left_out.update(out)
        for number, why in out.items():
            print(f"#{number}: left out: {why}")
        if not kept:
            return True
        stage("workspace-check")

        def build_refused(broken: str) -> bool:
            print(f"REFUSED: the batch does not build: {broken[:300]}")
            failed_build.append(broken)
            failed_units.clear()  # a scanner run beside a tree that does not build says nothing; the build is the reason
            return False

        def blame(new: list[str]) -> None:
            if any(name.startswith("integration docs") for name in new):
                keep_batch_evidence({number: infos[number]["headRefOid"] for number in group}, ROOT, Path(os.environ.get("LAND_TMP", "/tmp")) / "refused", time.strftime("%H%M%S"))
                for line in batch_blame({number: infos[number]["headRefOid"] for number in group}, BASE):
                    print(line)

        def tests_pass() -> bool:
            """The lib tests and scanners on the merged tree; False after printing the refusal and noting the failed units."""
            with_pinned_clang(os.environ, ROOT)
            # LAND_FAST_TESTS=1: also the lib tests of the crates the batch changes (the user's 10-19 call is no tests; this is the switch for the opposite call)
            if os.environ.get("LAND_FAST_TESTS") == "1":
                changed = git("diff", "--name-only", BASE, "HEAD").split()
                # The always-on scanners read sources and docs (milliseconds): a red one refuses the batch before the minutes of lib tests it would otherwise wait for.
                stage("scanners-early")
                if new := sorted(landing_scanner_failures(changed, False)):
                    print(f"REFUSED: the batch fails {', '.join(new)}, which the head passes{parity_smoke_module().causes_of(new)}{docs_detail(new, members=members_files)}{offender_detail(new, members_files)}")
                    failed_units.update(new)
                    blame(new)
                    return False
                stage("lib-tests")
                if new := sorted(new_lib_failures(changed)):
                    print(f"REFUSED: the batch fails {', '.join(new)}, which the head passes{parity_smoke_module().causes_of(new)}{docs_detail(new, members=members_files)}{offender_detail(new, members_files)}")
                    failed_units.update(new)
                    blame(new)
                    return False
                stage("scanners")
                if new := sorted(new_scanner_failures(changed)):
                    print(f"REFUSED: the batch fails {', '.join(new)}, which the head passes{parity_smoke_module().causes_of(new)}{docs_detail(new, members=members_files)}{offender_detail(new, members_files)}")
                    failed_units.update(new)
                    blame(new)
                    return False
            if os.environ.get("LAND_FAST_TESTS") != "1":
                stage("scanners")  # a cargo test build of the scanner targets: workspace-check's 58% of a batch hid it
            if os.environ.get("LAND_FAST_TESTS") != "1" and (new := sorted(landing_scanner_failures(git("diff", "--name-only", BASE, "HEAD").split(), False))):
                print(f"REFUSED: the batch fails {', '.join(new)}, which the head passes{parity_smoke_module().causes_of(new)}{docs_detail(new, members=members_files)}{offender_detail(new, members_files)}")
                failed_units.update(new)
                blame(new)
                return False
            if broken_tools := tool_test_failures(git("diff", "--name-only", BASE, "HEAD").split()):
                print(f"REFUSED: the batch fails the tests of the tools it changes: {' / '.join(broken_tools)[:300]}")
                failed_units.add("tools tests")
                return False
            return True

        def release_pass() -> tuple[list[str], list[str], list[str]]:
            """The link check, then the behavioural programs unless something does not link, then the stale oracle: (unlinked, red, stale)."""
            files = git("diff", "--name-only", BASE, "HEAD").split()
            stage("link")
            if unlinked := new_link_failures(files):
                return unlinked, [], []
            stage("behavioural")
            red = pre_merge_behavioural(files)
            if red:
                return [], red, []
            stage("stale")
            return [], [], pre_merge_stale(files)

        if OVERLAP:
            if broken := _workspace_check(ROOT):
                return build_refused(broken)
            tests_ok, (unlinked, red, stale) = beside(release_pass, tests_pass)
        else:
            # the dev-profile check (its own target dir) and the release scanner build share nothing: 60 s + 100 s in a row in sqb-5342
            why_serial = overlap_verdict()
            print(f"check beside scanners: {'serial (' + why_serial + ')' if why_serial else 'on'}")
            if not why_serial:
                broken, tests_ok = check_and_scan(lambda: _workspace_check(ROOT), tests_pass)
            else:
                broken = _workspace_check(ROOT)
                tests_ok = not broken and tests_pass()
            if broken:
                return build_refused(broken)
            unlinked, red, stale = release_pass() if tests_ok else ([], [], [])
        if not tests_ok:
            return False
        if unlinked:
            print(f"REFUSED: the batch's runtime does not link: {'; '.join(unlinked)[:300]}{offender_detail(unlinked, members_files, near=True)}")
            failed_units.update(f"link {line.split(':')[0]}" for line in unlinked)
            return False
        if red:
            print(f"BEHAVIOURAL RED pre-merge: the batch, {' '.join(red)}{red_env_cause()}")
            failed_units.update(f"behavioural {program}" for program in red)
            return False
        if stale:
            print(f"REFUSED: the batch fails LLRM_CHECK_STALE: {' / '.join(stale)[:600]}")
            failed_units.add("stale oracle")
            return False
        return True

    def merge_alone(number: str) -> bool:
        """PR `number` merged alone onto a clean head in the batch's checkout."""
        stage(f"alone #{number}")
        clean_checkout(BASE)
        git("fetch", "-q", "origin", infos[number]["headRefOid"])
        if subprocess.run(["git", "merge", "--no-edit", "-q", infos[number]["headRefOid"]], cwd=ROOT, capture_output=True).returncode:
            subprocess.run(["git", "merge", "--abort"], cwd=ROOT, capture_output=True)
            return False
        return True

    def merge_all(numbers: list[str]) -> bool:
        """The PRs `numbers` merged together onto a clean head in the batch's checkout."""
        stage(f"halve {len(numbers)}: {' '.join('#' + n for n in numbers)}")
        clean_checkout(BASE)
        for number in numbers:
            git("fetch", "-q", "origin", infos[number]["headRefOid"])
            if subprocess.run(["git", "merge", "--no-edit", "-q", infos[number]["headRefOid"]], cwd=ROOT, capture_output=True).returncode:
                subprocess.run(["git", "merge", "--abort"], cwd=ROOT, capture_output=True)
                return False
        return True

    def offending(group: list[str]) -> list[str]:
        """The PRs of `group` that fail what the batch failed, each merged alone on the head: the unit asked exactly (a minute at most per PR), or the build; none when nothing is known."""
        if failed_build:
            own = {}
            named_files = error_files(failed_build[-1])
            suspects = [number for number in group if named_files & set(head_files(infos[number]['headRefOid']))]
            culprits = build_offenders(group, merge_alone, lambda: _workspace_check(ROOT), own, suspects)
            named.update({number: f"alone on the head it does not build: {own[number][:160]}" for number in culprits})
            return culprits
        wrong = sorted(name[len("behavioural ") :] for name in failed_units if name.startswith("behavioural "))
        if wrong:
            files_of = lambda number: head_files(infos[number]['headRefOid'])  # noqa: E731
            culprits = behavioural_culprits(
                group,
                files_of,
                lambda number: merge_alone(number) and bool(pre_merge_behavioural(files_of(number))),
                lambda subset: merge_all(subset) and bool(pre_merge_behavioural(sorted({f for n in subset for f in files_of(n)}))),
            )
            for number in culprits:
                named[number] = f"BEHAVIOURAL RED pre-merge: {' '.join(wrong)}{red_env_cause()}"
                print(f"BEHAVIOURAL RED pre-merge: #{number} {' '.join(wrong)}{red_env_cause()}")
            return culprits
        if "stale oracle" in failed_units:
            # a finding the base has too is no PR's: only the kinds a PR adds name it
            clean_checkout(BASE)
            on_base = stale_kinds(pre_merge_stale(["crates/_base.rs"]))
            files_of = lambda number: head_files(infos[number]['headRefOid'])  # noqa: E731
            added = lambda files: bool(stale_kinds(pre_merge_stale(files)) - on_base)  # noqa: E731
            crate_prs = [number for number in group if any(f.startswith("crates/") for f in files_of(number))]
            culprits = red_members_suspects_first(
                group,
                crate_prs,
                lambda subset: merge_all(subset) and added(sorted({f for n in subset for f in files_of(n)})),
                lambda number: merge_alone(number) and added(files_of(number)),
            )
            for number in culprits:
                named[number] = "LLRM_CHECK_STALE fails with it"
                print(f"STALE RED pre-merge: #{number}")
            return culprits
        integration = {name[len("integration ") :].split("::", 1)[0] for name in failed_units if name.startswith("integration ")}
        if integration:
            # a test the base fails too is no PR's: every PR merged on a red base fails it alone
            clean_checkout(BASE)
            probe, targets = integration_probe(failed_units, parity_smoke_module().integration_failures(sorted(integration)))
            if not probe:
                return []
            red_with = lambda subset: merge_all(subset) and bool(probe & parity_smoke_module().integration_failures(targets))  # noqa: E731
            alone_red = lambda number: merge_alone(number) and bool(probe & parity_smoke_module().integration_failures(targets))  # noqa: E731
            failure_files = files_named_in(parity_smoke_module().causes_of(failed_units))
            suspects = [number for number in group if failure_files & set(head_files(infos[number]['headRefOid']))]
            return red_members_suspects_first(group, suspects, red_with, alone_red)
        units = {tuple(name[len("unit "):].split("::", 1)) for name in failed_units if name.startswith("unit ")}
        by_package: dict[str, set[str]] = {}
        for package, test in units:
            by_package.setdefault(package, set()).add(test)
        if not by_package:
            return []
        smoke = parity_smoke_module()
        packages, graph = smoke.workspace_graph()
        changed = {number: head_files(infos[number]['headRefOid']) for number in group}
        def probe(package: str, tests: set[str]) -> set[str]:
            LAST_PANICS.clear()  # a panic read for another PR's probe is not this one's reason
            return failing_on_checkout(package, tests)

        own: dict[str, list[str]] = {}
        reasons: dict[str, str] = {}
        culprits = unit_offenders(
            group,
            by_package,
            merge_alone,
            probe,
            base_red=_base_red,
            affects=lambda number, package: can_affect(changed[number], package, packages, graph),
            own=own,
            reasons=reasons,
            why=lambda names: "; ".join(
                filter(
                    None,
                    [
                        "; ".join(f"{test}: {LAST_PANICS[test]}" for test in (name[len("unit ") :].split("::", 1)[1] for name in names) if test in LAST_PANICS),
                        parity_smoke_module().causes_of(set(names)).strip(" []"),
                    ],
                )
            ),
        )
        named.update({number: f"alone on the head it fails {', '.join(own[number])[:160]}" + (f" [{reasons[number][:300]}]" if reasons.get(number) else "") for number in culprits})
        return culprits

    def passes(group: list[str]) -> bool:
        if batch_expired(started, time.monotonic()):
            raise BatchTooOld
        for number in defer_moved(infos, [one for one in group if one not in left_out], left_out):
            print(f"#{number}: {left_out[number]}")
        if FAST:
            return passes_fast(group)
        git("fetch", "-q", "origin", "feat/parity")
        head_failed = head_failures(git("rev-parse", BASE).strip(), lambda: compute_head_failures(known))
        git("checkout", "-q", "--detach", BASE)
        for number in group:
            git("fetch", "-q", "origin", infos[number]["headRefOid"])
            merge = subprocess.run(["git", "merge", "--no-edit", "-q", infos[number]["headRefOid"]], cwd=ROOT, capture_output=True, text=True, errors="replace")
            if merge.returncode and (conflicts := git("diff", "--name-only", "--diff-filter=U").split()) and not regenerate(conflicts):
                continue  # only generated docs conflicted: regenerated and committed
            if merge.returncode:
                subprocess.run(["git", "merge", "--abort"], cwd=ROOT, capture_output=True)
                print(f"#{number}: does not merge with the others: {merge.stdout.strip()[:120] or merge.stderr.strip()[:120]}")
                return False
        for number in group:
            for note in warnings(git("rev-parse", "HEAD").strip(), infos[number]["body"] or "", title=infos[number]["title"]):
                print(f"#{number}: WARNING: {note}")
            if why := refusals(git("rev-parse", "HEAD").strip(), infos[number]["body"] or "", title=infos[number]["title"]):
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

    rescan_check = (lambda *_: "") if os.environ.get("LAND_SKIP_RESCAN") == "1" else make_rescan_check()  # the user's bulk order: the batch scan stands, no rescan after a drop

    pending: list[str] = []

    def land(number: str) -> None:
        if number in left_out:
            if left_out[number].startswith("head moved"):
                print(f"#{number}: LEFT OUT (head moved): {left_out[number]}")
                moved.append(number)
                return
            print(f"#{number}: REFUSED: left out: {left_out[number]}")
            refused.append(number)
            return
        if waiting(number):  # before the formatted head is pushed to the PR's branch
            print(f"#{number}: HELD: in waits.txt now; not pushed")
            held.append(number)
            late_drops.append(number)
            return
        group = [one for one in getattr(land, "group", [number]) if one not in landed and one not in refused and one not in held]
        if why := rescan_check(number, infos, late_drops, group):
            print(f"#{number}: REFUSED: {why}")
            refused.append(number)
            late_drops.append(number)
            return
        if PUSH_MODE:
            pending.append(number)
            return
        finish(number)

    def finish(number: str) -> None:
            sha, why = format_head(infos[number]["headRefOid"], infos[number]["headRefName"])
            if not sha:
                print(f"#{number}: REFUSED: {why}")
                refused.append(number)
                late_drops.append(number)
                return
            if sha != infos[number]["headRefOid"]:
                print(f"#{number}: formatted with tools/fmt.sh ({sha[:9]})")
                infos[number] = {**infos[number], "headRefOid": sha}
                for _ in range(12):  # GitHub takes the pushed head as the PR's before a --match-head-commit squash accepts it
                    seen = json.loads(gh_run(["gh", "pr", "view", number, "--json", "headRefOid"], capture_output=True, text=True, errors="replace").stdout or "{}").get("headRefOid")
                    if seen == sha:
                        break
                    time.sleep(5)
            if number in docs_synced:
                synced_sha, why = sync_docs_head(infos[number]["headRefOid"], infos[number]["headRefName"])
                if not synced_sha:
                    print(f"#{number}: REFUSED: {why}")
                    refused.append(number)
                    late_drops.append(number)
                    return
                if synced_sha != infos[number]["headRefOid"]:
                    print(f"#{number}: generated docs synced with tools/docs/docsbuild/build.py --sync ({synced_sha[:9]})")
                    infos[number] = {**infos[number], "headRefOid": synced_sha}
                    for _ in range(12):
                        seen = json.loads(subprocess.run(["gh", "pr", "view", number, "--json", "headRefOid"], capture_output=True, text=True, errors="replace").stdout or "{}").get("headRefOid")
                        if seen == synced_sha:
                            break
                        time.sleep(5)
            landed_ok = squash(number, infos[number], known_red_line(known))
            if landed_ok is None:
                held.append(number)
                late_drops.append(number)
                return
            (landed if landed_ok else moved if number in HEAD_MOVED else refused).append(number)
            if not landed_ok:
                late_drops.append(number)
            if landed_ok:
                git("fetch", "-q", "origin", "feat/parity")
                log_landed(git("rev-parse", BASE).strip(), number, "gate-ok")

    with land_lock():
        if git("status", "--porcelain", "--untracked-files=no").strip():
            raise SystemExit("this checkout has uncommitted changes")
        try:
            batch(list(numbers), passes, land, refuse, offending if FAST else None, pending)
            if pending:
                tip, why = push_tip({n: infos[n]["headRefOid"] for n in pending})
                if tip:
                    print(f"PUSHED: {tip} to feat/parity with {' '.join('#' + n for n in pending)}")
                    for number in pending:
                        landed.append(number)
                        log_landed(tip, number, "gate-ok")
                else:
                    print(f"one-push failed ({why}); squashing each PR instead")
                    for number in pending:
                        finish(number)
        except BatchTooOld:
            back = [one for one in numbers if one not in landed and one not in refused]
            print(f"batch older than {BATCH_MAX_SECONDS // 60} min: its heads and assembly are stale; back to the queue unlanded: {' '.join('#' + one for one in back) or '-'}")
    for line in hold_own_refusals(refused, {**left_out, **named}, {n: infos[n]["headRefOid"] for n in refused if n in infos}):
        print(line)
    if times := stage_summary():
        print(times)
    print(batch_summary(landed, refused, held, moved))
    return 1 if refused else 0


def claims_fix(description: str) -> bool:
    """Whether the PR says it fixes a red: a line `Fixes red` / `Fixes a red ...` in its description."""
    return bool(FIXES_RED.search(description or ""))


def new_names(head: set[str], merged: set[str]) -> list[str]:
    """The failing names of `merged` that `head` does not fail. Names are per test or program (`ratchet:x::t`, `run p [-O2]`, `unit crate::t`), so a test that goes red inside a
    step that was red already is new. A bare step name in `head` (a failure set cached before steps named their tests, or a step whose output names none) covers its own."""
    return sorted(n for n in merged - head if not any(n.startswith((f"{h}::", f"{h} ")) for h in head))


def fixed_names(head: set[str], merged: set[str]) -> list[str]:
    """The failing names of `head` that `merged` no longer fails."""
    return sorted(head - merged)


def subset_refusals(head: set[str], merged: set[str], fixes: bool) -> list[str]:
    """Why the merged tree may not land on a head that fails `head`: it fails a check the head passes, or (a PR that says it fixes a red) fails as many as the head.
    The gate must not deadlock on a red head, and must not let a PR add a red. Both sets are in the reason."""
    shown = f"head fails {{{', '.join(sorted(head)) or '-'}}}, merged tree fails {{{', '.join(sorted(merged)) or '-'}}}"
    if new := new_names(head, merged):
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


LAST_PANICS: dict[str, str] = {}  # test name -> its first panic line, from the last `failing_on_checkout`


def failing_on_checkout(crate: str, tests: set[str], cwd: Path = ROOT, env: dict | None = None) -> set[str]:
    """Which of `tests` of `crate` fail on the checkout in `cwd` (exact names, release not needed: the smoke ran them as `cargo test -q --lib`)."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import parity_smoke
    from buildslot import slotted  # noqa: PLC0415

    command = parity_smoke.lib_command(crate)
    done = subprocess.run(slotted([*command, "--", "--exact", *sorted(tests)]), cwd=cwd, capture_output=True, text=True, errors="replace", env=env)
    LAST_PANICS.update(parity_smoke.panic_reasons(done.stdout + done.stderr))
    return {name for name in tests if name in set(parity_smoke.failed_tests(done.stdout + done.stderr))}


def run_streaming(command: list[str], cwd: Path):
    """The finished process (.returncode, .stdout, .stderr) with the stage lines of its stdout echoed as they arrive, so the squash's log has the smoke's step times."""
    import tempfile
    import types

    with tempfile.TemporaryFile("w+") as err:
        process = subprocess.Popen(command, cwd=cwd, stdout=subprocess.PIPE, stderr=err, text=True, errors="replace")
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
    # a head whose tests did not run says nothing about them: stored, it excused none of the head's real reds for good (277c2651b0, header_copy: batch 17)
    if any(name.endswith("did-not-run") for name in failed):
        return failed
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


def parity_smoke_module():
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import parity_smoke  # noqa: PLC0415

    return parity_smoke


def compute_head_qbrt_failures() -> set[str]:
    """The tests/qbrt differential's failing results on the head itself (binaries built), once per head sha."""
    stage("head-qbrt")
    git("checkout", "-q", "--detach", BASE)
    if stale := ensure_bins():
        raise SystemExit(f"the head's own binaries: {stale}")
    return parity_smoke_module().qbrt_failures(smoke_bins())


def compute_head_group_failures() -> set[str]:
    """The tests/run groups' failing programs on the head itself (check origin/feat/parity out, build its binaries when stale), once per head sha."""
    stage("head-groups")
    git("checkout", "-q", "--detach", BASE)
    if stale := ensure_bins():
        raise SystemExit(f"the head's own binaries: {stale}")
    return parity_smoke_module().group_failures(smoke_bins())


def compute_head_lib_failures(packages: set[str]) -> set[str]:
    """Every workspace lib test's failures on the head itself, once per head sha: a PR's dependants-lib run is compared with it by name."""
    stage("head-lib")
    git("checkout", "-q", "--detach", BASE)
    return parity_smoke_module().dependants_lib_failures(packages)


def compute_head_integration_failures(targets: list[str]) -> set[str]:
    """The failing tests of the same root integration `targets` on the head itself, once per head sha and target set."""
    stage("head-integration")
    git("checkout", "-q", "--detach", BASE)
    return parity_smoke_module().integration_failures(targets)


def compute_head_qcport_failures() -> set[str]:
    """Every QCport module's failing compiles on the head itself, once per head sha (the PR's own run is never cached)."""
    stage("head-qcport")
    git("checkout", "-q", "--detach", BASE)
    if stale := ensure_bins():
        raise SystemExit(f"the head's own binaries: {stale}")
    return parity_smoke_module().qcport_failures(smoke_bins())


def merge(number: str, known: dict[str, str] | None = None) -> int:
    with land_lock():
        return _merge(number, known)


def _merge(number: str, known: dict[str, str] | None = None) -> int:
    known = known or {}
    info = json.loads(gh_run(["gh", "pr", "view", number, "--json", "headRefOid,title,body,baseRefName"], capture_output=True, text=True, errors="replace", check=True).stdout)
    if info["baseRefName"] != "feat/parity":
        raise SystemExit(f"#{number} targets {info['baseRefName']}, not feat/parity")
    git("fetch", "-q", "origin", "feat/parity")
    if git("status", "--porcelain", "--untracked-files=no").strip():
        raise SystemExit("this checkout has uncommitted changes: the fast set runs on the PR head merged with feat/parity, checked out here")
    git("fetch", "-q", "origin", info["headRefOid"])
    tested_base = git("rev-parse", BASE).strip()
    head_failed = head_failures(tested_base, lambda: compute_head_failures(known))
    files = {f["path"] for f in json.loads(gh_run(["gh", "pr", "view", number, "--json", "files"], capture_output=True, text=True, errors="replace", check=True).stdout)["files"]}
    whole = parity_smoke_module().runs_whole_groups(files)
    if whole:
        head_failed = head_failed | head_failures(tested_base, compute_head_group_failures, cache=HEAD_FAILURES / "groups")
        head_failed = head_failed | head_failures(tested_base, compute_head_qbrt_failures, cache=HEAD_FAILURES / "qbrt")
    smoke = parity_smoke_module()
    packages, graph = smoke.workspace_graph()
    touched = smoke.touched_packages(files, packages)
    closure = smoke.dependants(touched, graph)
    targets = smoke.integration_targets_for(files, touched, smoke.integration_map())
    if targets:
        key = hashlib.sha1(" ".join(targets).encode()).hexdigest()[:12]
        head_failed = head_failed | head_failures(tested_base, lambda: compute_head_integration_failures(targets), cache=HEAD_FAILURES / "integration" / key)
    if closure:
        head_failed = head_failed | head_failures(tested_base, lambda: compute_head_lib_failures(set(packages)), cache=HEAD_FAILURES / "lib")
    compiler = parity_smoke_module().touches_the_compiler(files)
    if compiler:
        head_failed = head_failed | head_failures(tested_base, compute_head_qcport_failures, cache=HEAD_FAILURES / "qcport")
    merged, why = merged_result(info["headRefOid"])
    if merged is None:
        print(f"REFUSED: {why}")
        return 1
    for note in warnings(merged, info["body"] or "", title=info.get("title", "")):
        print(f"#{number}: WARNING: {note}")
    if why := refusals(merged, info["body"] or "", title=info.get("title", "")):
        print("\n".join(f"REFUSED: {w}" for w in why))
        return 1
    if stale := ensure_bins():
        print(f"REFUSED: {stale}")
        return 1
    merged_failed, last = smoke_failures(known)
    print(last)
    if whole:
        merged_failed = merged_failed | parity_smoke_module().group_failures(smoke_bins()) | parity_smoke_module().qbrt_failures(smoke_bins())
    if compiler:
        merged_failed = merged_failed | parity_smoke_module().qcport_failures(smoke_bins())
    if closure:
        merged_failed = merged_failed | smoke.dependants_lib_failures(closure)
    if targets:
        merged_failed = merged_failed | smoke.integration_failures(targets)
    if unit_tests_of(merged_failed - head_failed):
        git("checkout", "-q", "--detach", BASE)
        head_failed = head_expanded(head_failed, merged_failed, failing_on_checkout)
        git("checkout", "-q", "--detach", merged)
    if fixed := fixed_names(head_failed, merged_failed):
        print(f"FIXED: {', '.join(fixed)}")
    if why := subset_refusals(head_failed, merged_failed, claims_fix(info["body"] or "")):
        print("\n".join(f"REFUSED: {w}" for w in why))
        return 1
    if why := stale_at_the_call(merged):
        print("\n".join(f"REFUSED: {w}" for w in why))
        return 1
    if git("rev-parse", BASE).strip() != tested_base:
        print(f"REFUSED: feat/parity moved from {tested_base[:9]} to {git('rev-parse', BASE).strip()[:9]} while the checks ran; run again")
        return 1
    done = gh_run(["gh", "pr", "merge", number, "--squash", "--match-head-commit", info["headRefOid"], "--subject", info["title"], "--body", known_red_line(known)])
    if done.returncode:
        return done.returncode
    git("fetch", "-q", "origin", "feat/parity")
    squash = json.loads(gh_run(["gh", "pr", "view", number, "--json", "mergeCommit"], capture_output=True, text=True, errors="replace", check=True).stdout)["mergeCommit"]["oid"]
    git("fetch", "-q", "origin", squash)
    log_landed(squash, number, "gate-ok" + (f" (head red: {','.join(sorted(head_failed))})" if head_failed else ""))
    if strays := squash_strays(squash, files):
        print(f"REVERT-DETECTED: squash {squash[:9]} of #{number} touches files the PR never listed (a stale tree reverting other work?); restore them NOW:")
        print("\n".join(f"  {path} {change}" for path, change in strays))
        return 3
    return 0


TOOL_MODULES = ("parity_smoke", "xfail", "buildslot")  # what the landing imports lazily, after PRs are merged into the checkout


def preload_tools(names: tuple[str, ...] = TOOL_MODULES) -> None:
    """Import the tool modules before the first PR is merged into the checkout, so a landing that changes them (tools/parity_*.py) takes effect at the next batch, a new process,
    and never half-way through this one: #3817 changed `parity_smoke.lib_command` in the batch that held the old `parity_squash`, the merged-tree run used the feature, the head baseline did not,
    and the 15 known reds were refused as new."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    for name in names:
        importlib.import_module(name)


def main() -> int:
    """The landing commands run at the landing's build priority (`buildslot.landing`); the others (audit, unlogged) start no cargo."""
    preload_tools()
    from buildslot import LANDING, landing  # noqa: PLC0415

    if sys.argv[1:2] == ["alone"]:
        # the precheck of the next batch runs beside this one's tests: through buildslot like any cargo, not at the landing's priority (the runner exports BUILDSLOT_BYPASS=1,
        # and six of eight cargo processes at load 108 were alone-0..5 outside every slot)
        os.environ.pop("BUILDSLOT_BYPASS", None)
        os.environ.pop(LANDING, None)
        return dispatch()
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
    if len(sys.argv) >= 3 and sys.argv[1] == "alone":
        git("fetch", "-q", "origin", "feat/parity")
        numbers = [a for a in sys.argv[2:] if a.isdigit()]
        heads = {}
        for number in numbers:
            git("fetch", "-q", "origin", f"refs/pull/{number}/head")
            heads[number] = git("rev-parse", "FETCH_HEAD").strip()
        # stdout is the survivors and nothing else: the scanners print their summary line, and the runner reads this output as PR numbers
        with contextlib.redirect_stdout(sys.stderr):
            kept, dropped = alone(numbers, heads, cache=Path(os.environ.get("LAND_TMP", "/tmp")) / "alone-cache-v2", jobs=int(os.environ.get("ALONE_JOBS", ALONE_JOBS)), cap=float(os.environ.get("ALONE_CAP", "600")), current=_current_head, recheck=_alone_docs_recheck, base_sha=git("rev-parse", BASE).strip())
        for number, why in dropped.items():
            print(alone_refusal_line(number, why, heads[number], Path(os.environ.get("LAND_TMP", "/tmp")) / "alone-full"), file=sys.stderr)
        print(f"alone: {len(numbers)} PRs, {HITS[0]} cached, {len(numbers) - HITS[0] - len(UNCHECKED)} checked, {len(UNCHECKED)} UNCHECKED at the cap{' (' + ' '.join(UNCHECKED) + ')' if UNCHECKED else ''}, {len(dropped)} dropped", file=sys.stderr)
        print("\n".join(kept))
        return 0
    if len(sys.argv) >= 4 and sys.argv[1] == "assemble":
        git("fetch", "-q", "origin", "feat/parity")
        numbers = [a for a in sys.argv[3:] if a.isdigit()]
        heads = {}
        for number in numbers:
            git("fetch", "-q", "origin", f"refs/pull/{number}/head")
            heads[number] = git("rev-parse", "FETCH_HEAD").strip()
        started = time.monotonic()
        kept, waits = assemble(numbers, heads, limit=int(sys.argv[2]))
        print(f"{time.strftime('%H:%M:%S')} {PREFIX} assemble {time.monotonic() - started:.0f}s: {len(kept)} kept, {len(waits)} wait", file=sys.stderr)
        for number, why in waits.items():
            print(f"#{number}: waits a round: {why}", file=sys.stderr)
        print("\n".join(kept))
        return 0
    if len(sys.argv) >= 3 and sys.argv[1] == "merge-batch":
        numbers = [a for a in sys.argv[2:] if not a.startswith("--") and a.isdigit()]
        rest = sys.argv[2:]
        try:
            return merge_batch(numbers, parse_known_red([rest[i + 1] for i, a in enumerate(rest) if a == "--known-red" and i + 1 < len(rest)]))
        except Exception as error:
            for number in numbers:
                print(crash_line(number, error))
            raise
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
