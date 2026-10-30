"""The gate in tiers: pick the tier and steps a diff needs (tiers.toml), run them concurrently.

    gate.py plan [--base REF] [--files F...] [--tier auto|fast|full]
    gate.py run  [same]            needs CARGO_TARGET_DIR; JOBS=N sets the concurrency (default half the cores)
    gate.py plan --json            the plan as JSON: tier, the CI job groups with something to run, the skipped steps
    gate.py run --group G          only the steps of group G (tiers.toml [groups]), after the build; G=build: the build alone
                                   With GATE_ALLOW_MISSING=1 (CI) a step whose [capability] is missing on this host is SKIPPED with its reason, never run;
                                   without it a missing tool fails as it always did.
    gate.py bisect GOOD BAD STEP...  first commit on main (first-parent) where the named steps fail
    gate.py main [--force]       the scheduled full tier over origin/main: runs when 5 merges or 2 hours have passed since
                                 the last green; red -> bisects since that green. Needs CARGO_TARGET_DIR, a tree of its own.
"""

from __future__ import annotations

import ast
import os
import shutil
import signal
import functools
import re
import sys
import json
import time
import argparse
import tomllib
import subprocess
from pathlib import Path
from typing import Callable
from dataclasses import dataclass, field
from concurrent.futures import ThreadPoolExecutor, as_completed

import proctree

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE.parent))
import envflag  # noqa: E402
LANGUAGES = ("qb", "c", "nib")
TREE_ONLY = {"fmt", "docs", "products"}  # steps that read the tree and build nothing: the `static` group
NO_BINARIES = {"pytest", "pytest-perf", "fmt", "rfmt-post", "docs", "facts"}  # steps that do not run the release binaries, so need no build first
SERIAL_STEPS = frozenset({"rfmt-post", "qcport", "oracle-qcport"})  # the last two count frames in real time: starved by a step beside them, their runs differ from Borland's


@functools.cache  # read once: a bisect checks out commits that predate this file
def load() -> dict:
    return tomllib.loads((HERE / "tiers.toml").read_text())


def glob(pattern: str) -> re.Pattern:
    out, i = "", 0
    while i < len(pattern):
        if pattern.startswith("**/", i):
            out, i = out + "(?:.*/)?", i + 3
        elif pattern.startswith("**", i):
            out, i = out + ".*", i + 2
        elif pattern[i] == "*":
            out, i = out + "[^/]*", i + 1
        else:
            out, i = out + re.escape(pattern[i]), i + 1
    return re.compile(out + "$")


def matches(path: str, patterns: list[str]) -> bool:
    return any(glob(p).match(path) for p in patterns)


def packages() -> dict[str, dict]:
    """name -> {dir, deps}: the workspace's crates and what each depends on in the workspace."""
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    names = {p["name"] for p in meta["packages"]}
    return {
        p["name"]: {
            "dir": str(Path(p["manifest_path"]).parent.relative_to(ROOT)),
            "deps": {d["name"] for d in p["dependencies"] if d["name"] in names and d.get("kind") != "dev"},
        }
        for p in meta["packages"]
    }


def dependents(pkgs: dict[str, dict], touched: set[str]) -> set[str]:
    """`touched` and every crate that depends on one, through any chain."""
    out = set(touched)
    while grown := {n for n, p in pkgs.items() if p["deps"] & out} - out:
        out |= grown
    return out


def crate_of(path: str, pkgs: dict[str, dict]) -> str | None:
    best = max((n for n, p in pkgs.items() if p["dir"] != "." and (path == p["dir"] or path.startswith(p["dir"] + "/"))), key=lambda n: len(pkgs[n]["dir"]), default=None)
    return best


@dataclass
class Plan:
    tier: str
    reason: str
    steps: list[str] = field(default_factory=list)
    packages: list[str] | None = None  # None: the whole workspace
    languages: list[str] = field(default_factory=list)
    files: list[str] = field(default_factory=list)  # the changed files, kept for the steps that run what the diff touches (smoke)


def head_state() -> tuple[str, bool]:
    """The commit the tree is at and whether a tracked file differs from it."""
    head = subprocess.run(["git", "-C", str(ROOT), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    changed = subprocess.run(["git", "-C", str(ROOT), "status", "--porcelain", "--untracked-files=no"], capture_output=True, text=True, check=True).stdout.strip()
    return head, bool(changed)


def refusal(head: str, dirty: bool, expect: str | None, allow_dirty: bool) -> str | None:
    """Why a run must not start: the checkout it was asked for is not the head, or the tree is not the head's (a gate reports on
    a commit, and a pass on anything else names a commit that was not run)."""
    if expect and not head.startswith(expect):
        return f"HEAD is {head[:12]}, not the checkout asked for ({expect}): nothing was run"
    if dirty and not allow_dirty:
        return f"the tree differs from HEAD {head[:12]} in tracked files: commit or stash them (--allow-dirty runs it anyway, and says so)"
    return None


def verdict_line(tier: str, failed: list[str], cut: list[str], took: float, skipped: list[str], head: str) -> str:
    """The last line of a run: its verdict, at the commit it ran."""
    verdict = "FAIL: " + " ".join(failed) if failed else "PASS"
    return (
        f"GATE {tier} {verdict} at {head[:12]}"
        + (f" (INCOMPLETE: {' '.join(cut)})" if cut else "")
        + f" in {took:.0f}s"
        + (f" (skipped: {' '.join(skipped)})" if skipped else "")
    )


def failure_text(log: str, limit: int = 80) -> str:
    """What the failing tests of a cargo test or pytest log printed, at most `limit` lines a file (pytest_shards prints one
    `######## file: exit N` section per failing file): the log's last 30 lines name the failed target and not the failure (CI said
    "1 target failed: -p llrm --test dwarf" and nothing of why), and one cap over the whole log went to the files that sort first."""
    out = []
    for section in re.split(r"(?m)^(?=######## )", log):
        shown, keep = [], False
        for line in section.splitlines():
            if re.match(r"^---- .+ (stdout|stderr) ----$", line) or re.match(r"^_{3,} .+ _{3,}$", line) or re.match(r"^={3,} (FAILURES|ERRORS) ={3,}$", line):
                keep = True
            elif re.match(r"^(failures:|test result:|={3,} short test summary info ={3,})", line):
                keep = False
            if keep and line.strip():
                shown.append(line)
        if shown:
            out += ([section.splitlines()[0]] if section.startswith("########") else []) + shown[:limit]
    return "\n".join(out)


def diagnostic_lines(log: str, failed: bool) -> list[str]:
    """The lines a step tags for the job log: `[diag!] ` ones always (the host's CPU, the emulator, a verdict), `[diag] ` ones only when the step
    failed (hash tables). The job log shows a failed step's failing tests and the tail of its log, so a line printed before the tests was in
    the step's own log file and never reached it (run 6's dos step printed neither lscpu nor the x87 verdict)."""
    return [line for line in log.splitlines() if line.startswith("[diag!] ") or (failed and line.startswith("[diag] "))]


def log_head(log: str, lines: int = 12) -> str:
    """The first `lines` non-blank lines of a log: where a script that prints its verdict first (oracle-qcport's) says what failed,
    and the tail shows only the emulator's logs under it."""
    return "\n".join([line for line in log.splitlines() if line.strip()][:lines])


def step_table(results: dict[str, tuple[int, float, float]]) -> str:
    """Each step's wall and CPU seconds, the most CPU first: where the work is, whatever the host's load made of the wall."""
    rows = sorted(((n, w, c) for n, (code, w, c) in results.items() if code != 77), key=lambda r: -r[2])
    return "\n".join(["[steps] step wall(s) cpu(s)", *(f"[steps] {n} {w:.0f} {c:.0f}" for n, w, c in rows), f"[steps] total {sum(r[2] for r in rows):.0f} cpu"])


def changed_files(base: str) -> list[str]:
    out = subprocess.run(["git", "diff", "--name-only", base, "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return out.split()


def root_tests() -> list[str]:
    return sorted(p.stem for p in (ROOT / "tests").glob("*.rs"))


def crate_tests(pkgs: dict[str, dict]) -> list[tuple[str, str]]:
    out = []
    for name, p in sorted(pkgs.items()):
        if p["dir"] != ".":
            out += [(name, f.stem) for f in sorted((ROOT / p["dir"] / "tests").glob("*.rs"))]
    return out


def reading(cfg: dict, pkgs: dict[str, dict], live: list[str]) -> set[str]:
    """The crates whose own tests read other crates' sources (`[[reads]]`), for the files changed: a crate that reads what
    changed is not one that depends on it, so `dependents` leaves it out (facts_rewrite's audit of every reader of nsw/nuw
    went unrun when a pass in another crate began to read them)."""
    return {r["package"] for r in cfg.get("reads", []) if r["package"] in pkgs and any(matches(f, r["paths"]) for f in live)}


# the two names a commit must not carry, spelt in halves so this file does not carry them
MENTION = re.compile("clau" + "de|anthro" + "pic", re.I)


def refusals(root: Path = ROOT, base: str = "origin/main", pr: dict | None = None) -> list[str]:
    """Why a branch is not for the smoke tier (and the train): each reason on a line, none when it may go on. `pr` is `gh pr view --json
    baseRefName,title,body` where the PR is known."""
    def git_out(*args: str) -> subprocess.CompletedProcess:
        return subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)

    out = []
    if git_out("status", "--porcelain", "--untracked-files=no").stdout.strip():
        out.append("the tree differs from HEAD")
    if pr is not None and pr.get("baseRefName") != "main":
        out.append(f"the PR's base is {pr.get('baseRefName')!r}, not main: a stacked PR waits for its parent to land")
    if git_out("merge-base", "--is-ancestor", base, "HEAD").returncode != 0:
        # not on top of main: behind it, and merging must not conflict
        if git_out("merge-tree", "--write-tree", base, "HEAD").returncode != 0:
            out.append(f"HEAD conflicts with {base}")
    text = git_out("log", "--format=%B", f"{base}..HEAD").stdout
    added = [l[1:] for l in git_out("diff", "--unified=0", f"{base}...HEAD").stdout.splitlines() if l.startswith("+") and not l.startswith("+++")]
    if pr is not None:
        text += f"\n{pr.get('title', '')}\n{pr.get('body', '')}"
    if MENTION.search(text):
        out.append("a commit message or the PR text names the assistant or its maker")
    if any(MENTION.search(l) for l in added):
        out.append("the diff adds a line naming the assistant or its maker")
    return out


def pull_request(number: int | None) -> dict | None:
    if number is None:
        return None
    done = subprocess.run(["gh", "pr", "view", str(number), "--json", "baseRefName,title,body"], capture_output=True, text=True)
    if done.returncode:
        sys.exit(f"gate: gh pr view {number}: {done.stderr.strip()}")
    return json.loads(done.stdout)


def smoke_dir() -> Path:
    return Path(os.environ.get("LLRM_SMOKE_DIR") or Path.home() / ".cache/llrm/smoke")


def record_smoke(sha: str) -> None:
    """A whole smoke run passed at `sha`: the train takes only PRs whose head has this."""
    smoke_dir().mkdir(parents=True, exist_ok=True)
    (smoke_dir() / sha).write_text(f"{time.time():.0f}\n")


def smoked(sha: str) -> bool:
    return (smoke_dir() / sha).is_file()


# The check of tests/tracked_build_products.rs without a build, for smoke (a test ties the lists).
PRODUCTS = ("obj", "exe", "o", "map", "lst")
FIXTURES = ("tests/inputs/", "tests/fixtures/")


def strays(names: list[str]) -> list[str]:
    return [n for n in names if n.rpartition(".")[2].lower() in PRODUCTS and "." in n and not n.startswith(FIXTURES)]


def link_strays(root: Path) -> list[str]:
    """What `git add -A` takes from a build tree: a tracked symlink pointing outside the repository or at an absolute path (a `target` link to a
    cargo target dir made #2171 red for hours: "tree differs from HEAD"), or a build dir at the root. Relative links inside the tree are fine
    (tests/run/nib/errors.nib)."""
    entries = subprocess.run(["git", "ls-files", "-s"], cwd=root, capture_output=True, text=True, check=True).stdout.splitlines()
    found = []
    for entry in entries:
        meta, _, name = entry.partition("\t")
        if meta.startswith("120000"):
            target = os.readlink(root / name)
            inside = not os.path.isabs(target) and (root / name).parent.joinpath(target).resolve().is_relative_to(root.resolve())
            if not inside:
                found.append(f"tracked symlink {name} -> {target}")
        elif name == "target" or name.startswith("target/"):
            found.append(f"tracked build dir {name}")
    return found


def products_main(root: Path | None = None) -> int:
    root = root or ROOT
    listed = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True).stdout.split("\n")
    if found := strays([n for n in listed if n]):
        print(f"build products are tracked: {found}")
        return 1
    if links := link_strays(root):
        print("\n".join(links))
        return 1
    return 0


def smoke_plan(files: list[str]) -> Plan:
    """The fast tier's plan cut to the smoke steps, plus docs and the diff's own tests."""
    fast = plan(files)  # narrowed when no compiler source changed
    if fast.tier == "full":
        fast = plan(files, "fast")
    keep = load()["smoke"]["steps"]
    built = "build" in fast.steps
    # the audit of target facts is integration's in the fast tier: the smoke tier runs it alone
    steps = [s for s in keep if s in ("docs", "changed-tests", "products") or s in fast.steps or s in ("pytest-programs", "pytest-quick") and built or (s == "facts" and "integration" in fast.steps)]
    return Plan("smoke", "the fast tier cut to the smoke steps", steps, fast.packages, fast.languages, list(files))


def touches_compiler(files: list[str], forced: str = "auto") -> bool:
    """Whether the diff changes the compiler's source (tiers.toml [compiler]): what the perf-counter tests and measure judge."""
    cfg = load()
    return forced != "auto" or any(matches(f, cfg["compiler"]["paths"]) for f in files if not matches(f, cfg["inert"]["paths"]))


def plan(files: list[str], forced: str = "auto") -> Plan:
    if forced == "smoke":
        return smoke_plan(files)
    cfg = load()
    pkgs = packages()
    live = [f for f in files if not matches(f, cfg["inert"]["paths"])]
    # Nothing of the compiler changed: the steps that cover the files that did, whatever the shared tests and tools are.
    narrow = forced == "auto" and not any(matches(f, cfg["compiler"]["paths"]) for f in live)
    shared = [] if narrow else [f for f in live if matches(f, cfg["shared"]["paths"])]
    unknown = [f for f in live if f not in shared and not matches(f, cfg["plain"]["paths"])]
    if forced != "auto":
        tier, reason = forced, "asked for"
    elif shared or unknown:
        tier, reason = "full", "shared: " + ", ".join((shared + unknown)[:3]) + (" ..." if len(shared + unknown) > 3 else "")
    else:
        tier, reason = "fast", "no shared path touched"
    p = Plan(tier, reason)
    if not live and forced == "auto":
        p.tier, p.reason = "none", "only docs and *.md changed"
        return p

    touched = {crate_of(f, pkgs) for f in live} - {None}
    cargo = any(not f.startswith("tools/") or f.startswith("tools/bench/") for f in live)
    p.packages = None if p.tier == "full" else sorted(dependents(pkgs, touched) | {"llrm"} | reading(cfg, pkgs, live))  # the root crate enables features (llrm-c's toolchain) the others lack alone
    if narrow:
        return narrowed(p, cfg, live)
    steps = []
    if cargo or p.tier == "full":
        steps += ["build", "lib", "doc", "integration", "crate-tests", "bench", "torture", "oracle-sweep", "oracle-premise"]
    steps += ["pytest", "pytest-perf"]
    if cargo and p.tier != "full" and any(matches(f, cfg["scan_inputs"]["paths"]) for f in live):
        steps.append("scans")
    if not cargo and p.tier != "full" and any(f.startswith("tools/torture/") for f in live):
        steps += ["torture"]
    if p.tier == "full" or any(matches(f, cfg["fmt"]["paths"]) for f in live):
        steps.append("fmt")
    heavy = {}
    for h in cfg["heavy"]:
        heavy[h["step"]] = h["owners"]
    for name, owners in heavy.items():
        if p.tier == "full" or any(matches(f, owners) for f in live):
            steps.append(name)
    run_affecting = [f for f in live if not f.startswith("tools/") or f.startswith(("tools/dosbatch/", "tools/linkrecipe"))]
    if "build" in steps or p.tier == "full":
        steps += list(cfg["exclusive"])
    if run_affecting or p.tier == "full":
        steps.append("run")
        langs = set()
        for f in run_affecting:
            hit = {lang for lang, pats in cfg["run_languages"].items() if matches(f, pats)}
            langs |= hit or set(LANGUAGES)
        p.languages = list(LANGUAGES) if p.tier == "full" else [l for l in LANGUAGES if l in langs]
    if p.tier == "full":
        steps += plan(files, "fast").steps  # full is fast's steps and more: whatever fast adds for a diff, full has too
    # Every step but the plain Python tests runs the release binaries: they must be built, and current, first.
    if any(one not in NO_BINARIES for one in steps) and "build" not in steps:
        steps.insert(0, "build")
    p.steps = list(dict.fromkeys(steps))
    return p


def data_dir(path: str) -> str | None:
    """The directory of inputs `path` belongs to (bench/, examples/, tests/run/, runtime/, ...): what other files read it by, or None for code."""
    top, _, rest = path.partition("/")
    if top in ("crates", "src", "tools", "docs") or not rest:
        return None
    return "tests/run" if path.startswith("tests/run/") else top if top != "tests" else None


def step_of_source(path: str, cfg: dict) -> str | None:
    """The step that runs the tests in `path`, or None where none does."""
    name = Path(path).stem
    if path.startswith("tests/") and path.endswith(".rs") and "/" not in path[len("tests/"):]:
        whole = {**{v: k for k, v in cfg["whole"].items()}, **{v: k for k, v in cfg["exclusive"].items()}}
        return whole.get(name, "integration")
    if path.startswith("crates/") and "/tests/" in path and path.endswith(".rs"):
        return "crate-tests"
    if path.startswith("crates/") and path.endswith(".rs"):
        return "lib"
    if path.endswith(".py") and (path.startswith("tests/") or Path(path).name.startswith("test_")):
        return "pytest-programs" if name in ("test_programs_compile", "test_loops", "test_innerloops") else "pytest-perf" if path in perf_files() else "pytest"
    return None


@functools.cache
def sources() -> dict[str, str]:
    """Every file a step's tests are in, with its text: the Rust and Python files under tests/ and crates/, and the tests under tools/."""
    out = {}
    for pattern in ("tests/*.rs", "tests/*.py", "crates/**/*.rs", "tools/**/test_*.py"):
        for f in sorted(ROOT.glob(pattern)):
            if "target" not in f.parts:
                out[str(f.relative_to(ROOT))] = f.read_text(errors="replace")
    return out


def readers(directory: str, cfg: dict) -> set[str]:
    """The steps whose tests name `directory` (as `"bench"` or `bench/...`): the steps that read what is in it, found in the tests and not
    listed. A benchmark merged on its own runs broke pytest-programs, measure and the integration tests that iterate bench/ (#1458)."""
    mention = re.compile(rf"(?<![\w./-]){re.escape(directory)}(?:/|[\"'])")
    return {step for path, text in sources().items() if mention.search(text) and (step := step_of_source(path, cfg))}


def narrowed(p: Plan, cfg: dict, live: list[str]) -> Plan:
    """The steps for a diff with none of the compiler's source: each covers the files it was written for. The binaries are built first
    where a step runs them."""
    steps = ["pytest", "pytest-perf"]
    if any(f.startswith("tests/") and (f.endswith(".rs") or f.startswith("tests/common/")) or matches(f, cfg["integration_inputs"]["paths"]) for f in live):
        steps.append("integration")
    # tests/target_facts.rs reads every .py and .sh under tools/, crates/ and src/ (a tools-only PR landed four copies of a target fact):
    # that one binary, with no release build of the others.
    elif any(matches(f, cfg["scan_inputs"]["paths"]) for f in live):
        steps.append("facts")
    for directory in sorted({d for f in live if (d := data_dir(f))}):
        steps += sorted(readers(directory, cfg))
    # bench/ and the bench tool are its inputs, and so is the runtime the programs link: the bq32 rows count the kernel's calls into it.
    if any(f.startswith(("bench/", "tools/bench/", "runtime/")) for f in live):
        steps.append("bench")
    if any(f.startswith("tools/torture/") for f in live):
        steps.append("torture")
    if any(f in ("tools/oracle_sweep.py", "tools/oracle_expected.toml") for f in live):
        steps += ["oracle-sweep", "oracle-premise"]  # a change to the sweep runs the sweep: it cannot be tried any other way
    if any(matches(f, cfg["fmt"]["paths"]) for f in live):
        steps.append("fmt")
    for h in cfg["heavy"]:
        if any(matches(f, h["owners"]) for f in live):
            steps.append(h["step"])
    # The Rust tests under tests/ are the integration step's; the programs under tests/run/ are the run step's.
    run_affecting = [
        f
        for f in live
        if (not f.startswith("tools/") and not (f.startswith("tests/") and (f.endswith(".rs") or f.startswith("tests/common/"))))
        or f.startswith(("tools/dosbatch/", "tools/linkrecipe"))
    ]
    if run_affecting:
        steps.append("run")
        langs = set()
        for f in run_affecting:
            langs |= {lang for lang, pats in cfg["run_languages"].items() if matches(f, pats)} or set(LANGUAGES)
        p.languages = [l for l in LANGUAGES if l in langs]
    p.reason = "no compiler source changed"
    if any(one not in NO_BINARIES for one in steps):
        steps.insert(0, "build")
    p.steps = list(dict.fromkeys(steps))
    return p


def restricted(p: Plan, names: list[str], known: set[str]) -> Plan:
    """`p` with only the steps `names` (the tier, the languages and the crates as planned). A step the diff did not select may be named:
    a change to the measurement tools re-runs build and measure, not the other fourteen steps. The build comes
    first where a step needs the binaries, as `plan` has it."""
    unknown = [n for n in names if n not in known]
    if unknown:
        raise SystemExit(f"gate: no step named {' '.join(unknown)}; the steps are {' '.join(sorted(known))}")
    steps = [n for n in dict.fromkeys(names) if n != "build"]
    if any(one not in NO_BINARIES for one in steps) or "build" in names:
        steps.insert(0, "build")
    return Plan(p.tier, p.reason + " (steps asked for)", steps, p.packages, p.languages, p.files)


# The build every step runs after, and every measurement is taken with: `cargo build --bins` alone produces a different llrm-c (the
# test build unifies features differently), whose compile costs differ by up to 6% a step. tools/measure.py builds a base with it too.
# So there is no `cargo build --bins` here: it made a second set of every crate (238 CPU-s) for binaries the test build then links over
# (all of them but hir-mir, which the corpus scripts build themselves).
WARNING_FLAGS = "-D warnings"
WARNINGS_AS_ERRORS = f"RUSTFLAGS='{WARNING_FLAGS}'"
# Every step runs with the same RUSTFLAGS: a `cargo test --test X` without them rebuilt llrm-c apart from the build step's and put
# its binary over target/release/llrm-c (the inode changed after `integration`), while `torture`, `run` and the like were running it:
# "No such file or directory: .../release/llrm-c" in torture. One set of flags, one build, one binary.
# RUSTC_BOOTSTRAP too, for the lib step's `--shuffle`: libtest reads it when the tests run, but cargo takes it for an input of every
# crate it builds, so set on the lib step alone it rebuilt the whole workspace there (1,088 CPU-s) and again in the next cargo step.
# CARGO_INCREMENTAL=0 as CI builds: profile.release is incremental (256 units), and the same commit counts 10.3 -> 33.3 Minstr in lir pressuresink
# there, 11.3 -> 37.6 built as CI does, 11.9 -> 40.7 under dist: test_budgets (budgets recorded in CI's build) read a debt as paid locally (#2240).
STEP_ENV = {"RUSTFLAGS": WARNING_FLAGS, "RUSTC_BOOTSTRAP": "1", "CARGO_INCREMENTAL": "0"}
BUILD = (
    f"{WARNINGS_AS_ERRORS} cargo check --workspace --all-targets -q && "
    f"{WARNINGS_AS_ERRORS} cargo check --release --workspace --all-targets -q && "
    f"{WARNINGS_AS_ERRORS} cargo test --release -q --workspace --no-run"
)
# Measurements compare two revisions, so their historical base is built without
# today's warning policy.
MEASURE_BUILD = "env -u RUSTFLAGS cargo build --release -q --bins && env -u RUSTFLAGS cargo test --release -q --workspace --no-run"
# The shipped build (Cargo.toml `[profile.dist]`): what the creep run on main measures. Not a gate step: three minutes cold.
DIST_BUILD = "cargo build --profile dist -q --bins"


# What the doc step reports where no doc comment has a fence to run: a log with no result reads as a cut-short run.
SKIPPED_DOC = "echo 'test result: ok. 0 passed; 0 failed; no doc comment has a fence to run, rustdoc not run'"

# A lib test that depends on the order tests run in passes in the plain order and fails when the threads start in
# another (llrm-c's m16 tests, 29 of 136, when an m32 test bound the register file first). The lib step runs the
# tests in a random order; each test binary prints its seed ("running 136 tests (shuffle seed: N)", kept in the
# failed step's log), and LLRM_SHUFFLE_SEED=N repeats that order. (RUSTC_BOOTSTRAP is in STEP_ENV.)


def shuffle() -> str:
    seed = os.environ.get("LLRM_SHUFFLE_SEED")
    return f"--shuffle-seed {int(seed)}" if seed else "--shuffle"


def changed_tests(files: list[str]) -> str:
    """The command running the tests the diff touches: a test_*.py it changed or that sits beside a .py it changed (test_X.py for X.py),
    and the root test binaries tests/NAME.rs it changed. `true` where there are none."""
    python, binaries = [], []
    for f in files:
        path = ROOT / f
        if not f.endswith(".py") or not path.exists():
            if f.startswith("tests/") and f.endswith(".rs") and "/" not in f[len("tests/"):] and path.exists():
                binaries.append(path.stem)
            continue
        beside = path.with_name(path.name if path.name.startswith("test_") else f"test_{path.name}")
        if beside.exists() and f.startswith(("tools/", "crates/", "tests/")):
            python.append(str(beside.relative_to(ROOT)))
    commands = []
    if python:
        commands.append("PYTHONPATH=tools/gate${PYTHONPATH:+:$PYTHONPATH} uv run -q --project tools python tools/gate/pytest_shards.py " + " ".join(sorted(set(python))) + " -- -q -p no:cacheprovider -p pytest_cpulimit -p pytest_qcport")
    if binaries:
        commands.append("cargo test --release -q --no-fail-fast " + " ".join(f"--test {b}" for b in sorted(set(binaries))))
    return " && ".join(commands) or "true"


# Commands. Each runs under bash in the repo root with CARGO_TARGET_DIR set.
def commands(p: Plan, cfg: dict, pkgs: dict[str, dict], skip_bins: frozenset[str] = frozenset(), skip_py: tuple[str, ...] = ()) -> dict[str, str]:
    scope = "--workspace" if p.packages is None else " ".join(f"-p {n}" for n in p.packages)
    # The root crate has no library: a diff that selects it alone (a bench tool) has no lib or doc tests to run.
    # (with another crate that has one, the root crate stays in the scope: it turns on the features the others need)
    libs = scope if p.packages is None or any((ROOT / pkgs[n]["dir"] / "src/lib.rs").exists() for n in p.packages) else ""
    cargo = "cargo test --release -q --no-fail-fast"
    # The root tests are named with -p llrm: a workspace member's test target of the same name (llrm-arm64's `run`) runs under a bare `--test run` too.
    split = cfg["split"]
    whole = set(cfg["whole"].values()) | set(cfg["exclusive"].values())
    skips = " ".join(f"--skip {s['filter']}" for s in split)
    cheap_bins = " ".join(f"--test {t}" for t in root_tests() if t != "timing" and t not in whole and t not in skip_bins)
    selected = set(p.packages or pkgs)
    ct = " ".join(f"-p {n} --test {t}" for n, t in crate_tests(pkgs) if n in selected)
    bench = (
        f'out=$(uv run -q --project tools python tools/bench/bench.py ${{BENCH_SHARD:+--shard "$BENCH_SHARD" --json "$CARGO_TARGET_DIR/bench-shards"}}); echo "$out" | grep -aE "^(FAIL|SKIP|BASE[A-Z-]*:? )|measurements:"; '
        'echo "$out" | tail -1 | grep -q " 0 problems"'
    )
    def cfg_pytest(skipped: tuple[str, ...], durations: str = "pytest-durations") -> str:
        # pytest-programs' two files are named out of the list: an explicit file argument runs though --ignore names it (the step ran them
        # twice, 160 s of a loaded host)
        own = {"tests/test_programs_compile.py", "tests/test_loops.py"}
        tests = " ".join(sorted(f"tests/{one.name}" for one in (ROOT / "tests").glob("*.py") if f"tests/{one.name}" not in own | set(skipped)))
        return f"PYTHONPATH=tools/gate${{PYTHONPATH:+:$PYTHONPATH}} uv run -q --project tools python tools/gate/pytest_shards.py --durations ${{CARGO_TARGET_DIR:-target}}/{durations}.json tools crates {tests}" + "".join(f" --ignore={f}" for f in skipped) + " -- -q -p no:cacheprovider -p pytest_cpulimit -p pytest_qcport"

    steps = {
        "build": BUILD,
        "facts": f"{cargo} -p llrm --test target_facts",
        # #1777 tracked d1.o, d4.o and d8.o and was smoke-green: the check of tests/tracked_build_products.rs, without a build.
        "products": "python3 -c \"import sys; sys.path.insert(0, 'tools/gate'); import gate; sys.exit(gate.products_main())\"",
        "docs": "python3 tools/docs/docsbuild/build.py --check && python3 tools/docs/passdocs.py --check",
        "changed-tests": changed_tests(p.files),
        "lib": f"{cargo} {libs} --lib -- -Zunstable-options {shuffle()}" if libs else "true",
        # rustdoc over every crate costs 456 CPU-s to find no test: it runs when a doc comment has a fence to run.
        "doc": f"if python3 tools/gate/doctests.py crates src tests; then {cargo} {libs} --doc; else {SKIPPED_DOC}; fi" if libs else "true",
        "integration": f"{cargo} -p llrm {cheap_bins} -- {skips} --skip test_every_program_under_tests_run_prints_its_out",
        "crate-tests": f"{cargo} {ct}" if ct else "true",
        # Every scan runs, and any failing fails the step: the first red must not hide the others.
        "scans": "rc=0; "
        + "; ".join(f"{cargo} -p {one['package']} " + (f"--lib -- {one['lib']}" if "lib" in one else f"--test {one['test']}") + " || rc=1" for one in cfg["scan"] if "package" in one)
        # The smoke tier runs no integration step: the scans that are root test binaries (seeded_maps escaped it in #1792) are run here
        # (target_facts is the facts step's).
        + "".join(f"; {cargo} --test {Path(one['file']).stem} || rc=1" for one in cfg["scan"] if p.tier == "smoke" and one.get("step") == "integration" and Path(one["file"]).stem != "target_facts")
        + "; exit $rc",
        "bench": bench,
        "torture": "uv run -q --project tools python tools/torture/torture.py --gate --work $CARGO_TARGET_DIR/torture-work",
        # Every LLRM_CHECK_* oracle over the BASIC and Nib programs (needs perf: it first checks each oracle ran). Red on a finding nothing
        # expects, or on an expected one that stopped failing: tools/oracle_expected.toml.
        "oracle-sweep": 'uv run -q --project tools python tools/oracle_sweep.py --no-premise --jobs ${JOBS:-8} --expected tools/oracle_expected.toml ${ORACLE_SHARD:+--shard "$ORACLE_SHARD"}',
        # Which oracle ran at all: measured with perf (tinytim), once; the sharded compiles need no counters and run on hosted runners.
        "oracle-premise": "uv run -q --project tools python tools/oracle_sweep.py --premise-only",
        # Enforced or not (tiers.toml [fmt]), the check runs and lists the files. Unenforced, "files would change" (exit 1) passes
        # and anything else (2 or more: no toolchain, a failed build, a crash) still fails the step.
        "fmt": "tools/fmt.sh --check"
        if cfg["fmt"]["enforced"]
        else "tools/fmt.sh --check; rc=$?; [ $rc -le 1 ] || exit $rc; [ $rc = 0 ] || echo 'fmt: not enforced yet (tiers.toml [fmt])'",
        "rfmt-post": 'cargo test --release -q --manifest-path tools/rfmt-post/Cargo.toml --target-dir "${CARGO_TARGET_DIR:-target}/rfmt-post"',
        "pytest": cfg_pytest(skip_py + tuple(perf_files())),
        # The tests that read hardware counters, alone: the one job that needs a runner with them (tiers.toml [requires]). On the dist compiler,
        # the build the cost gate measures (tools/measure.py).
        "pytest-perf": "LLRM_BIN=$(python3 tools/measure.py dist) PYTHONPATH=tools/gate${PYTHONPATH:+:$PYTHONPATH} uv run -q --project tools python tools/gate/pytest_shards.py --durations ${CARGO_TARGET_DIR:-target}/pytest-perf-durations.json "
        + " ".join(perf_files())
        + " -- -q -p no:cacheprovider -p pytest_cpulimit -p pytest_qcport",
        # The slow DOS tier (tools/dosbatch/conftest.py): the demos played on dos32 and the graphics sweeps. The demo sources are
        # Microsoft's and not in the repository, so without them the step is skipped (77), not passed.
        "dos-slow": 'd="${QB45_DEMOS_DIR:-$HOME/scratch/gor}"; [ -f "$d/NIBBLES.BAS" ] || { echo "SKIPPED: no NIBBLES.BAS in $d (set QB45_DEMOS_DIR)"; exit 77; }; rm -f "${CARGO_TARGET_DIR:-target}/gate-logs/demo-hashes.txt"; { lscpu | grep -E "Vendor ID|Model name"; rustc -vV | head -n 4; "${DOSBOX_BIN:-${CARGO_TARGET_DIR:-target}/release/dosbox-x}" --version 2>&1 | grep -m1 "DOSBox-X version"; uv run -q --project tools python -c "import sys; sys.path.insert(0, \'tools/dosbatch\'); import qb32; print(\'demo conf:\', \', \'.join(l for l in qb32.DEMO_CONF.splitlines() if l.startswith((\'core=\', \'cputype=\', \'virtual\'))))"; uv run -q --project tools python tools/dosbatch/x87probe.py || true; readelf -p .comment "${DOSBOX_BIN:-${CARGO_TARGET_DIR:-target}/release/dosbox-x}" | grep -m1 "GCC:" || true; } 2>&1 | sed \'s/^/[diag!] /\'; QB45_DEMOS_DIR="$d" uv run -q --project tools python tools/gate/pytest_shards.py --must-pass tools/dosbatch/test_qbdemos.py --rerun tools/dosbatch/test_qbdemos.py --durations ${CARGO_TARGET_DIR:-target}/dos-durations.json tools/dosbatch -- -m slow_dos -q -p no:cacheprovider; rc=$?; QB45_DEMOS_DIR="$d" uv run -q --project tools python tools/dosbatch/nibprobe.py 2>&1 | sed \'s/^/[diag!] /\'; sed \'s/^/[diag] /\' "${CARGO_TARGET_DIR:-target}/gate-logs/demo-hashes.txt" 2>/dev/null; exit $rc',
        # What the full tier's pytest step runs but for its slow files: #1765 and #1766 were smoke-green and failed tools/test_identical.py
        # and tests/test_readme.py in the train.
        "pytest-quick": cfg_pytest(skip_py + tuple(cfg["smoke"]["slow_pytest"]), "pytest-quick-durations"),
        "pytest-programs": "uv run -q --project tools python -m pytest tests/test_programs_compile.py tests/test_loops.py -q -p no:cacheprovider",
        "qcport": '[ -f ~/scratch/qcport-env.sh ] && . ~/scratch/qcport-env.sh; : "${QCPORT:?QCPORT is not set}" "${QCPORT_INC:?QCPORT_INC is not set}" "${QCPORT_BORLAND:?QCPORT_BORLAND is not set}"; export JWLINK="${JWLINK:-$LLRM_BIN/jwlink}"; uv run -q --project tools python tools/qcport-run-cc.py',
        "oracle-qcport": '[ -f ~/scratch/qcport-env.sh ] && . ~/scratch/qcport-env.sh; : "${QCPORT:?QCPORT is not set}" "${QCPORT_INC:?QCPORT_INC is not set}" "${QCPORT_BORLAND:?QCPORT_BORLAND is not set}"; export JWLINK="${JWLINK:-$LLRM_BIN/jwlink}"; uv run -q --project tools python tools/oracle_qcport.py --jobs ${JOBS:-8}',
        "measure": "[ -f ~/scratch/qcport-env.sh ] && . ~/scratch/qcport-env.sh; python3 tools/measure.py check",
        "run": f"LLRM_RUN_ONLY='{' '.join(p.languages)}' {cargo} -p llrm --test run -- test_every_program_under_tests_run_prints_its_out",
    }
    for s in split:
        steps[s["step"]] = f"{cargo} -p llrm --test {s['bin']} -- {s['filter']}"
    for step, binary in {**cfg["whole"], **cfg["exclusive"]}.items():
        steps[step] = f"{cargo} -p llrm --test {binary}"
    return steps


def incomplete(text: str, binaries: int | None, filtered: bool) -> str | None:
    """Why a cargo test log shows a run cut short, or None. A cut run prints fewer results, not failures."""
    results = re.findall(r"^test result: \w+\. (\d+) passed", text, re.M)
    running = len(re.findall(r"^running \d+ tests?", text, re.M))
    if not results:
        return "no test result"
    if len(results) < running:
        return f"{len(results)} results of {running} test binaries"
    if binaries is not None and len(results) != binaries:
        return f"{len(results)} results of {binaries} test binaries"
    if filtered and not sum(map(int, results)):
        return "its filter matched no test"
    return None


def expected(p: Plan, cfg: dict, pkgs: dict[str, dict], skip_bins: frozenset[str] = frozenset()) -> dict[str, tuple[int | None, bool]]:
    """(test binaries that must report, whether a filter must match a test) per cargo test step."""
    whole = set(cfg["whole"].values()) | set(cfg["exclusive"].values())
    selected = set(p.packages or pkgs)
    libs = p.packages is None or any((ROOT / pkgs[n]["dir"] / "src/lib.rs").exists() for n in p.packages)
    out = {
        **({"lib": (None, False), "doc": (None, False)} if libs else {}),
        "integration": (sum(1 for t in root_tests() if t != "timing" and t not in whole and t not in skip_bins), False),
        "facts": (1, False),  # one binary that must report: a log with none (a build that ran no test) is incomplete, not a pass
        "run": (1, True),
        "scans": (None, True),
        "rfmt-post": (None, False),
    }
    ct = sum(1 for n, _ in crate_tests(pkgs) if n in selected)
    if ct:
        out["crate-tests"] = (ct, False)
    for s in cfg["split"]:
        out[s["step"]] = (1, True)
    out.update({step: (1, False) for step in {**cfg["whole"], **cfg["exclusive"]}})
    return out


def missing_capabilities(environ: dict | None = None) -> dict[str, str]:
    """capability -> why this host lacks it: a directory its [capability] table names is not there.
    Only where GATE_ALLOW_MISSING is set (the CI workflow): elsewhere a moved tool directory must turn the gate red, not skip.
    "1" allows every capability to be missing; a comma list ("perf") allows those and no others: a tool CI should have turns red.
    One whose LLRM_REQUIRE_* variable the caller set is never missing: its tests fail, as they always did."""
    environ = os.environ if environ is None else environ
    out = {}
    if not environ.get("GATE_ALLOW_MISSING"):
        return out
    allowed = environ["GATE_ALLOW_MISSING"]
    for name, cap in load().get("capability", {}).items():
        if allowed != "1" and name not in allowed.split(","):
            continue
        if any(envflag.flag(v, environ) for v in cap["require"]):
            continue
        for need in cap["needs"]:
            if "run" in need:  # a probe: the command must succeed
                if subprocess.run(need["run"], shell=True, capture_output=True, env=dict(environ)).returncode:
                    out[name] = f"{name}: `{need['run']}` fails here"
                    break
                continue
            where = Path((environ.get(need["env"]) or need["default"]).replace("~", environ.get("HOME", "~"), 1))
            if not (where / need["marker"]).exists():
                out[name] = f"{name}: {where / need['marker']} not found (set {need['env']})"
                break
    return out


def skipped_steps(steps: list[str], missing: dict[str, str], languages: list[str] | None = None) -> dict[str, str]:
    """step -> reason, for each step that needs a capability this host lacks. `run` also when none of its `languages` can run."""
    needs = load().get("requires", {})
    out = {s: "; ".join(missing[c] for c in needs[s] if c in missing) for s in steps if any(c in missing for c in needs.get(s, []))}
    if "run" in steps and languages and not [l for l in languages if l not in unusable(missing)[0]]:
        out["run"] = "no run language can run here: " + "; ".join(missing.values())
    return out


@functools.cache
def declared_requirements(root: Path = ROOT) -> dict[str, list[str]]:
    """Python test files -> the capabilities they declare: a module-level `REQUIRES = ["perf"]` beside the read that needs it."""
    files = subprocess.run(["git", "ls-files", "--cached", "--others", "--exclude-standard", "*test_*.py"], cwd=root, capture_output=True, text=True).stdout.split()
    out = {}
    for f in files:
        text = (root / f).read_text()
        if "REQUIRES" in text:
            for node in ast.parse(text).body:
                if isinstance(node, ast.Assign) and [t.id for t in node.targets if isinstance(t, ast.Name)] == ["REQUIRES"]:
                    out[f] = list(ast.literal_eval(node.value))
    return out


def perf_files() -> list[str]:
    """The Python test files that declare `REQUIRES = ["perf"]`: the pytest-perf step's, left out of pytest."""
    return sorted(f for f, needs in declared_requirements().items() if "perf" in needs)


def assumed(missing: dict[str, str], environ) -> dict[str, str]:
    """`missing` without the capabilities GATE_PLAN_ASSUME names: a plan is for the jobs' runners, not for the host that makes it."""
    return {c: why for c, why in missing.items() if c not in environ.get("GATE_PLAN_ASSUME", "").split(",")}


def python_tests_unusable(missing: dict[str, str], root: Path = ROOT) -> list[str]:
    """Python test files whose own REQUIRES names a capability this host lacks."""
    return sorted(f for f, needs in declared_requirements(root).items() if any(c in missing for c in needs))


def unusable(missing: dict[str, str]) -> tuple[list[str], frozenset[str]]:
    """(run languages, cheap test binaries) that need a capability this host lacks."""
    out = {k: [c for c in needs if c in missing] for k, needs in load().get("requires", {}).items()}
    langs = [k[5:] for k, hit in out.items() if hit and k.startswith("lang:")]
    return langs, frozenset(k[4:] for k, hit in out.items() if hit and k.startswith("bin:"))


# Groups cut into slices that run on their own runners: group -> (the tiers.toml table holding `shards`, the env variable the slice is passed in).
SHARDED = {"bench": ("bench", "BENCH_SHARD"), "oracle": ("oracle", "ORACLE_SHARD")}


# Slices a sharded group has besides its numbered ones: `bench-base-I` measures slice I of the base, for the numbered slices' rows to be judged against.
EXTRA_SLICES = {"bench": ("base",)}


def shard_names(group: str) -> list[str]:
    """`bench` as the CI jobs `bench-base-1` .. `bench-base-N`, `bench-1` .. `bench-N` (likewise `oracle`, without a base): the step is cut into slices that run on their own runners; any other group as itself."""
    if group not in SHARDED:
        return [group]
    count = load()[SHARDED[group][0]]["shards"]
    return [f"{group}-{one}-{i}" for one in EXTRA_SLICES.get(group, ()) for i in range(1, count + 1)] + [f"{group}-{i}" for i in range(1, count + 1)]


def shard_of(group: str | None) -> tuple[str | None, str | None]:
    """(the group, its slice "I/N") for a job named `bench-I` or `oracle-I`, (`bench`, "base:I/N") for `bench-base-I`; any other group as it is."""
    if group and (one := re.fullmatch(r"bench-base-(\d+)", group)):
        return "bench", f"base:{one[1]}/{load()['bench']['shards']}"
    if group and (one := re.fullmatch(r"(bench|oracle)-(\d+)", group)):
        return one[1], f"{one[2]}/{load()[SHARDED[one[1]][0]]['shards']}"
    return group, None


def private_groups(p: Plan, root: Path = ROOT) -> list[str]:
    """The groups of this plan that need a capability only the private checkout has: their jobs read it, and a fork's pull request fails them."""
    cfg = load()
    private = {name for name, cap in cfg["capability"].items() if cap.get("private")}
    steps = {"lang": "run", "bin": "integration"}  # the step that runs a language or a cheap test binary
    need = {steps.get(key.split(":")[0], key) if ":" in key else key for key, caps in cfg["requires"].items() if private & set(caps)}
    if any(private & set(caps) for caps in declared_requirements(root).values()):
        need |= {"pytest", "pytest-programs", "pytest-quick", "changed-tests"}
    return sorted(one for g, members in cfg["groups"].items() if any(s in p.steps and s in need for s in members) for one in shard_names(g))


def private_env(directory: str, group: str = "") -> dict[str, str]:
    """The tool variables for a job: each points into the private checkout at `directory`; the group's table replaces the general one."""
    cfg = load()["private"]
    return {name: str(Path(directory) / sub) for name, sub in {**cfg["env"], **cfg.get("group_env", {}).get(group, {})}.items()}


def groups_of(p: Plan, skipped: dict[str, str]) -> dict[str, list[str]]:
    """The CI job groups that have a step to run: group -> its steps in the plan, minus the skipped."""
    return {one: run for g, members in load()["groups"].items() if (run := [s for s in members if s in p.steps and s not in skipped]) for one in shard_names(g)}


STALL_CODE = 79


def stall_seconds() -> float:
    """How long a step may go with no log growth and no CPU in its process tree: GATE_STALL_MINUTES (10)."""
    return float(os.environ.get("GATE_STALL_MINUTES") or 10) * 60


LIVE: set[int] = set()  # process groups of the running steps


def kill_on_term() -> None:
    """SIGTERM and SIGHUP kill the running steps' process groups before the gate dies: it used to leave them running, writing to their logs."""

    def die(signum, frame):
        for pgid in list(LIVE):
            try:
                os.killpg(pgid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        os._exit(128 + signum)

    for sig in (signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, die)


# Steps whose log is also printed as it grows: the job log keeps a step's lines only if it ends, and a step cut by the job timeout left
# none (gate run 7: bench-base cancelled at 90 minutes, no stage line to say where). A test step's thousands of lines stay in its file.
STREAMED = ("bench",)


class LogTail:
    """The complete lines a step's log has gained since the last pump(), printed as `[step] line`."""

    def __init__(self, path: Path, name: str):
        self.path, self.name, self.at, self.rest = path, name, 0, b""

    def pump(self) -> None:
        if not self.path.is_file():
            return
        with open(self.path, "rb") as log:
            log.seek(self.at)
            data = log.read()
        self.at += len(data)
        *lines, self.rest = (self.rest + data).split(b"\n")
        for line in lines:
            print(f"[{self.name}] " + line.decode(errors="replace"), flush=True)


def run_step(name: str, command: str, logs: Path, env: dict, check: tuple[int | None, bool] | None = None, unset: tuple[str, ...] = (), stall: float | None = None) -> tuple[str, int, float, float]:
    """(name, exit code, wall seconds, CPU seconds): the CPU is user+sys of the step's process tree, which a loaded host does not stretch.
    A step that neither writes to its log nor uses CPU for `stall` seconds is killed (a pytest step sat 84 minutes on a hung test with
    nothing to say so); it ends as STALL_CODE with the reason at the end of its log."""
    start = time.time()
    stall = stall_seconds() if stall is None else stall
    # debug-gate.env makes every tool a requirement; a capability this host lacks is not one.
    prelude = ". tools/debug-gate.env 2>/dev/null; " + "".join(f"unset {v}; " for v in unset) + "set -o pipefail; "
    path = logs / f"{name}.log"
    # A new file, not a truncated one: an orphan of a killed gate still writing to the old inode interleaves with this step's output
    # (NUL gaps and extra "test result" lines: #1758 "95 results of 83 test binaries").
    path.unlink(missing_ok=True)
    tail = LogTail(path, name) if name in STREAMED else None
    with open(path, "w") as log:
        child = subprocess.Popen(["bash", "-c", prelude + command], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        LIVE.add(child.pid)
        seen, active, used = -1, time.time(), 0.0
        stalled = False
        try:
            while True:
                # wait4 reports the usage of the child and of every descendant it waited for: the step's whole tree.
                done, status, usage = os.wait4(child.pid, os.WNOHANG)
                if done:
                    break
                time.sleep(min(1.0, stall / 4))
                if tail:
                    tail.pump()
                grown, burned = path.stat().st_size, proctree.tree_cpu(child.pid)
                if grown != seen or burned > used:  # any tick: a loaded host gives a computing step far less than 0.1 s a poll
                    seen, used, active = grown, burned, time.time()
                elif time.time() - active > stall:
                    doing = proctree.describe(child.pid)  # before the kill: what it was waiting on is the only evidence of the stall
                    os.killpg(child.pid, signal.SIGKILL)
                    _, status, usage = os.wait4(child.pid, 0)
                    stalled = True
                    break
        except BaseException:
            os.killpg(child.pid, signal.SIGKILL)
            raise
        LIVE.discard(child.pid)
        if tail:
            tail.pump()
        child.returncode = code = STALL_CODE if stalled else os.waitstatus_to_exitcode(status)
        cpu = usage.ru_utime + usage.ru_stime
    if stalled:
        path.open("a").write(f"\nSTALLED: no log growth and no CPU in the step's process tree for {stall / 60:g} min; killed\nthe tree when it was killed:\n" + "\n".join(doing) + "\n")
    if code == 0 and check and (why := incomplete(path.read_text(), *check)):
        path.open("a").write(f"\nINCOMPLETE: {why}\n")
        code = 78
    if code == 77 and os.environ.get("GATE_ALLOW_MISSING"):
        # A step that skips itself (no env file, no demos) on a runner meant to have its tools did not run: the missing ones allowed were
        # skipped by the plan, before this. CI's `qcport` said SKIPPED and passed.
        path.open("a").write("\nFAILED: the step skipped itself where only the capabilities in GATE_ALLOW_MISSING may be missing\n")
        code = 1
    if code not in (0, 77):
        # The next run of the step overwrites its log; the failure is evidence, kept.
        (logs / f"{name}.failed.log").write_text(path.read_text())
    return name, code, time.time() - start, cpu


def snapshot_binaries(release: Path, into: Path) -> Path:
    """The executables cargo left in `release`, linked into `into` for the steps to run. Every `cargo test --release --test X` compiles
    the root package's binaries again as a test does (another metadata hash) and links `release/llrm-c` to its own: cargo removes the file
    and links the new one, even for a unit that was fresh, so with steps running cargo side by side the path has moments with no file
    (torture: `No such file .../release/llrm-c`). A hard link keeps the inode it was made of: the next link cargo makes is another file."""
    into.mkdir(parents=True, exist_ok=True)
    for one in sorted(release.iterdir()):
        if one.is_file() and os.access(one, os.X_OK) and one.suffix != ".d":
            staged = into / f".{one.name}.new"
            staged.unlink(missing_ok=True)
            os.link(one, staged)
            os.replace(staged, into / one.name)
    return into


# The steps that run the compiler's binaries (the others build what they test, or read the tree).
USES_BINARIES = ("pytest", "pytest-perf", "pytest-programs", "pytest-quick", "changed-tests", "bench", "run", "torture", "oracle-sweep", "oracle-premise", "oracle-qcport", "identity", "measure", "qcport", "turbo", "cv4", "dos-slow")


def prebuilt(target: str | None = None) -> bool:
    """Whether this checkout's target dir is the build job's: GATE_PREBUILT is set by CI after it restores the shared target, and the
    commit that job wrote (target/.built-from) must be this one, with its release dir present. A flag over any other target builds."""
    target = target or os.environ.get("CARGO_TARGET_DIR", "target")
    built = Path(target) / ".built-from"
    return os.environ.get("GATE_PREBUILT") == "1" and (Path(target) / "release").is_dir() and built.is_file() and built.read_text().strip() == head_state()[0]


def group_plan(p: Plan, group: str) -> Plan:
    """`p` cut to the steps of CI job group `group` ("build": the build alone, for the job that makes what the others share). A group of
    checks that read the tree has no target dir to reuse, and gets no build: it would be the workspace's, from cold. A group handed the
    build job's target (`prebuilt`) gets none either: its copy of the build took 600-800 s in every job of gate run 8."""
    members = [] if group == "build" else load()["groups"][group]
    builds = group == "build" or (any(m not in TREE_ONLY for m in members) and not prebuilt())
    return Plan(p.tier, p.reason, [s for s in p.steps if (s == "build" and builds) or s in members], p.packages, p.languages, p.files)


def execute(p: Plan, group: str | None = None) -> tuple[int, list[str]]:
    group, shard = shard_of(group)
    target = os.environ.get("CARGO_TARGET_DIR")
    if not target:
        sys.exit("gate: CARGO_TARGET_DIR is not set")
    # The run test compares the tree before and after; a tool writing a .pyc into it meanwhile is not a leak.
    env = {**os.environ, **STEP_ENV, "PYTHONDONTWRITEBYTECODE": "1", "LLRM_BIN": os.environ.get("LLRM_BIN", f"{target}/release")}
    if shard:
        env[SHARDED[group][1]] = shard
    logs = Path(target) / "gate-logs"
    logs.mkdir(parents=True, exist_ok=True)
    pkgs = packages()
    missing = missing_capabilities()
    no_langs, skip_bins = unusable(missing)
    languages, p.languages = p.languages, [l for l in p.languages if l not in no_langs]
    cmds, checks = commands(p, load(), pkgs, skip_bins, tuple(python_tests_unusable(missing))), expected(p, load(), pkgs, skip_bins)
    unset = tuple(v for c in missing for v in load()["capability"][c]["require"])
    skipped = skipped_steps(p.steps, missing, languages)
    if group:
        p = group_plan(p, group)
    start = time.time()
    results: dict[str, tuple[int, float, float]] = {}

    def report(name: str, code: int, took: float, cpu: float = 0) -> None:
        results[name] = (code, took, cpu)
        verdict = "ok" if code == 0 else "SKIPPED" if code == 77 else "INCOMPLETE" if code == 78 else "FAIL"
        print(f"[time] {name} {verdict} {took:.0f}s wall {cpu:.0f}s cpu", flush=True)
        if code != 77 and (logs / f"{name}.log").exists():
            if tagged := diagnostic_lines((logs / f"{name}.log").read_text(), code != 0):
                print("\n".join("    " + line for line in tagged), flush=True)
        if code not in (0, 77):
            text = (logs / f"{name}.log").read_text()
            if failures := failure_text(text):
                print(f"    [{name}: what the failing tests printed]\n" + "\n".join("    " + line for line in failures.splitlines()), flush=True)
            else:
                print(f"    [{name}: the head of its log]\n" + "\n".join("    " + line for line in log_head(text).splitlines()), flush=True)
            print("\n".join("    " + line for line in text.splitlines()[-30:]), flush=True)

    for name, why in skipped.items():
        if name in p.steps:
            (logs / f"{name}.log").write_text(f"SKIPPED: {why}\n")
            report(name, 77, 0)
    p.steps = [s for s in p.steps if s not in skipped]
    for key, needs in load().get("requires", {}).items():
        if key.startswith(("lang:", "bin:")) and (hit := [missing[c] for c in needs if c in missing]):
            print(f"[dropped] {key}: {'; '.join(hit)}", flush=True)
    for f in python_tests_unusable(missing):
        print(f"[dropped] {f}: REQUIRES {', '.join(declared_requirements()[f])}", flush=True)
    for name, why in missing.items():
        print(f"[unavailable] {why}: its tests in other steps skip; run languages {no_langs}, test binaries {sorted(skip_bins)} left out", flush=True)
    if "build" in p.steps:
        report(*run_step("build", cmds["build"], logs, env, unset=unset))
        if results["build"][0]:
            print(verdict_line(p.tier, ["build"], [], time.time() - start, [], head_state()[0]))
            return 1, ["build"]
    if env["LLRM_BIN"] == f"{target}/release":
        if not (Path(target) / "release").is_dir():
            # A plan with no build assumes the binaries are there (a tools-only diff): on a fresh target dir say so, not FileNotFoundError.
            if needing := [s for s in p.steps if s in USES_BINARIES]:
                print(f"gate: {target}/release does not exist and this plan builds nothing; its steps ({' '.join(needing)}) run the compiler's binaries: "
                      "build them first (`gate.py run --group build`, or `cargo build --release --bins`), or set LLRM_BIN", flush=True)
                return 2, ["build"]
        else:
            env["LLRM_BIN"] = str(snapshot_binaries(Path(target) / "release", Path(target) / "gate-bins"))
    alone = [s for s in p.steps if s in load()["exclusive"] or s in SERIAL_STEPS]
    rest = [s for s in p.steps if s != "build" and s not in alone]
    jobs = default_jobs(os.environ)
    with ThreadPoolExecutor(jobs) as pool:
        # The longest steps start first.
        order = sorted(rest, key=lambda s: s not in ("run", "identity", "measure", "pytest-programs", "turbo", "bench", "dos-slow"))
        for future in as_completed([pool.submit(run_step, s, cmds[s], logs, env, checks.get(s), unset) for s in order]):
            report(*future.result())
    for name in alone:
        report(*run_step(name, cmds[name], logs, env, checks.get(name), unset))
    failed = [n for n, (c, *_) in results.items() if c not in (0, 77)]
    skipped = [n for n, (c, *_) in results.items() if c == 77]
    cut = [n for n in failed if results[n][0] == 78]
    print(step_table(results), flush=True)
    print(verdict_line(p.tier, failed, cut, time.time() - start, skipped, head_state()[0]))
    return (1 if failed else 0), failed


def first_bad(commits: list[str], fails: Callable[[str], bool]) -> str:
    """The first of `commits` (oldest first) that fails, given the last one does and a failure stays once it starts."""
    lo, hi = -1, len(commits) - 1  # commits[hi] fails; everything up to commits[lo] passes
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if fails(commits[mid]):
            hi = mid
        else:
            lo = mid
    return commits[hi]


def bisect(good: str, bad: str, steps: list[str]) -> int:
    """First first-parent commit in good..bad where `steps` fail (they pass at good)."""
    commits = subprocess.run(["git", "rev-list", "--first-parent", "--reverse", f"{good}..{bad}"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()

    def fails(commit: str) -> bool:
        subprocess.run(["git", "checkout", "-q", "--detach", commit], cwd=ROOT, check=True)
        print(f"-- {commit[:9]}", flush=True)
        return execute(Plan("full", "bisect", steps=["build", *steps], languages=list(LANGUAGES)))[0] != 0

    print(f"FIRST BAD: {first_bad(commits, fails)}")
    return 0


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()


def watch_main(force: bool, every: int = 5, hours: float = 2.0) -> int:
    """Full tier over origin/main once `every` merges have landed, or `hours` have passed with any since the last green; red bisects since that green."""
    target = os.environ.get("CARGO_TARGET_DIR")
    if not target:
        sys.exit("gate: CARGO_TARGET_DIR is not set")
    state_file = Path(target) / "gate-main.json"
    state = json.loads(state_file.read_text()) if state_file.exists() else {}
    git("fetch", "-q", "origin")
    git("checkout", "-q", "--detach", "origin/main")
    git("clean", "-ffdxq")
    head, green = git("rev-parse", "HEAD"), state.get("green")
    # Every commit of main this run sees gets its measurement (tools/measure.py): the base of the next branch from it.
    if not list((Path(os.environ.get("LLRM_MEASURE_DIR") or Path.home() / ".cache/llrm/measure")).glob(f"{head}-*.json")):
        full = plan(["Cargo.toml"], "full")
        execute(restricted(full, ["measure"], set(commands(full, load(), packages())) | set(load()["exclusive"])))
    # And against the main commit 50 merges or a week back: a branch may add up to the tolerance, ten of them may not.
    # Both ends of the creep are built as the shipped llrm-c is (`dist`): the slow drift is the user's, not the gate's build's.
    subprocess.run(["bash", "-c", DIST_BUILD], cwd=ROOT, env=os.environ, check=True)
    creep = subprocess.run([sys.executable, "tools/measure.py", "creep", "HEAD"], cwd=ROOT, env={**os.environ, "LLRM_BIN": f"{target}/dist", "LLRM_MEASURE_PROFILE": "dist"})
    if creep.returncode == 1:
        return 1
    merges = len(git("rev-list", "--first-parent", f"{green}..{head}").split()) if green else every
    if not (force or merges >= every or merges and time.time() - state.get("when", 0) >= hours * 3600):
        print(f"main: {merges} merges since the last green; not due")
        return 0
    code, failed = execute(plan(["Cargo.toml"], "full"))
    if code == 0:
        state_file.write_text(json.dumps({"green": head, "when": time.time()}))
        return 0
    if green:
        git("checkout", "-q", "--detach", head)
        bisect(green, head, [s for s in failed if s != "build"] or ["build"])
        git("checkout", "-q", "--detach", head)
    return 1


def default_jobs(environ: dict, cpus: int | None = None) -> int:
    """Steps at once: JOBS, else half the cores (at least 4). The fixed 4 left a 32-core host idle behind the four longest steps."""
    cpus = os.cpu_count() or 8 if cpus is None else cpus
    return int(environ.get("JOBS") or max(4, cpus // 2))


def scoped(argv: list[str], environ: dict | None = None, have: str | None = None) -> list[str] | None:
    """`argv` under its own systemd user scope with a high CPU weight, or None when it already is (GATE_SCOPED) or the host has no
    systemd-run, or runs under GitHub Actions (a runner is no dev box: the self-hosted podman ones have no user bus, and the gate refused in 0 s).
    Session builds share one scope with the gate by default and starve it; the weight is GATE_CPU_WEIGHT (1000). The scope is
    omitted from systemd-oomd: it killed two gates (SIGKILL, no verdict) for the host's memory pressure, not for their memory."""
    environ = os.environ if environ is None else environ
    have = shutil.which("systemd-run") if have is None else have
    if environ.get("GATE_SCOPED") or environ.get("GITHUB_ACTIONS") or not have or environ.get("GATE_CPU_WEIGHT") == "0":
        return None
    weight = environ.get("GATE_CPU_WEIGHT") or "1000"
    return [have, "--user", "--scope", "-q", "-p", f"CPUWeight={weight}", "-p", "ManagedOOMPreference=omit", "--setenv=GATE_SCOPED=1", *argv]


def scope_refusal(environ: dict | None = None, have: str | None = None) -> str | None:
    """Why `scoped` would fail: systemd-run --user needs a user bus. Without one it exited with only a dbus line and no verdict."""
    environ = os.environ if environ is None else environ
    if scoped(["x"], environ, have) is None or environ.get("XDG_RUNTIME_DIR") or environ.get("DBUS_SESSION_BUS_ADDRESS"):
        return None
    return "no systemd user bus (XDG_RUNTIME_DIR and DBUS_SESSION_BUS_ADDRESS unset): set GATE_SCOPED=1 to run without the scope"


def main() -> int:
    kill_on_term()
    ap = argparse.ArgumentParser()
    ap.add_argument("command", choices=["plan", "run", "bisect", "main", "private-env", "private-repo", "private-public", "shards"])
    ap.add_argument("rest", nargs="*")
    ap.add_argument("--base", default="origin/main")
    ap.add_argument("--files", nargs="*")
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--expect", metavar="SHA", help="refuse to run unless HEAD starts with this: the commit the run is to report on")
    ap.add_argument("--allow-dirty", action="store_true", help="run on a tree that differs from HEAD (the verdict is then of no commit)")
    ap.add_argument("--pr", type=int, help="smoke: the PR number, whose base and text are checked too (needs gh)")
    ap.add_argument("--group")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--tier", default="auto", choices=["auto", "fast", "full", "smoke"])
    ap.add_argument("--steps", nargs="+", metavar="STEP", help="only these steps (the plan is otherwise as planned); `build measure`")
    args = ap.parse_args()
    if args.command == "main":
        return watch_main(args.force)
    if args.command in ("private-repo", "private-public"):
        cfg = load()["private"]
        print(cfg["repo"] if args.command == "private-repo" else "\n".join(f"{k}={v}" for k, v in cfg["public"].items()))
        return 0
    if args.command == "shards":  # for the merge job: how many bench shards to wait for
        print(load()["bench"]["shards"])
        return 0
    if args.command == "private-env":  # for `>> "$GITHUB_ENV"`: paths only, nothing read from the checkout
        for name, value in private_env(args.rest[0], args.group or "").items():
            print(f"{name}={value}")
        return 0
    if args.command == "bisect":
        return bisect(args.rest[0], args.rest[1], args.rest[2:])
    if args.command == "run" and args.tier == "smoke":
        if why := refusals(pr=pull_request(args.pr)):
            print("gate: smoke refuses:\n  " + "\n  ".join(why))
            return 2
    if args.command == "run":
        head, dirty = head_state()
        if (why := refusal(head, dirty, args.expect, args.allow_dirty)) is not None:
            print(f"gate: {why}")
            return 2
        if not args.json and (why := scope_refusal()):
            print(f"gate: {why}")
            return 2
        if not args.json and (wrapped := scoped([sys.executable, *sys.argv])):
            os.execv(wrapped[0], wrapped)  # before the first line is printed: the re-executed run prints it
        print(f"gate: HEAD {head} {'dirty (allowed)' if dirty else 'clean'}", flush=True)
    files = args.files if args.files is not None else changed_files(args.base)
    p = plan(files, args.tier)
    if args.steps:
        p = restricted(p, args.steps, set(commands(p, load(), packages())) | set(load()["exclusive"]))
    missing = missing_capabilities()
    if args.command == "plan":  # the jobs' runners may have what the planner's host lacks (perf counters); a job checks for itself
        missing = assumed(missing, os.environ)
    skipped = skipped_steps(p.steps, missing, p.languages)
    if args.json:
        print(json.dumps({"tier": p.tier, "steps": p.steps, "groups": groups_of(p, skipped), "private": private_groups(p), "skipped": skipped, "compiler": touches_compiler(files, args.tier)}))
        return 0
    print(f"tier {p.tier}: {p.reason}")
    for step, why in skipped.items():
        print(f"SKIPPED {step}: {why}")
    print("steps:", " ".join(p.steps), "| run languages:", " ".join(p.languages), "| crates:", "all" if p.packages is None else f"{len(p.packages)}")
    if args.command == "plan" or p.tier == "none":
        return 0
    code = execute(p, args.group)[0]
    if code == 0 and p.tier == "smoke" and not args.steps and not args.group:
        record_smoke(head_state()[0])
    return code


if __name__ == "__main__":
    sys.exit(main())
