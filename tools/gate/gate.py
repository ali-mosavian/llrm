"""The gate in tiers: pick the tier and steps a diff needs (tiers.toml), run them concurrently.

    gate.py plan [--base REF] [--files F...] [--tier auto|fast|full]
    gate.py run  [same]            needs CARGO_TARGET_DIR; JOBS=N sets the concurrency (default 4)
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

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
LANGUAGES = ("qb", "c", "nib")
NO_BINARIES = {"pytest", "fmt", "rfmt-post"}  # steps that do not run the release binaries, so need no build first
SERIAL_STEPS = frozenset({"rfmt-post"})


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


def plan(files: list[str], forced: str = "auto") -> Plan:
    cfg = load()
    pkgs = packages()
    live = [f for f in files if not matches(f, cfg["inert"]["paths"])]
    shared = [f for f in live if matches(f, cfg["shared"]["paths"])]
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
    p.packages = None if p.tier == "full" else sorted(dependents(pkgs, touched) | {"llrm"})  # the root crate enables features (llrm-c's toolchain) the others lack alone
    steps = []
    if cargo or p.tier == "full":
        steps += ["build", "lib", "doc", "integration", "crate-tests", "bench", "torture"]
    steps += ["pytest"]
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
    return Plan(p.tier, p.reason + " (steps asked for)", steps, p.packages, p.languages)


# The build every step runs after, and every measurement is taken with: `cargo build --bins` alone produces a different llrm-c (the
# test build unifies features differently), whose compile costs differ by up to 6% a step. tools/measure.py builds a base with it too.
WARNINGS_AS_ERRORS = "RUSTFLAGS='-D warnings'"
BUILD = (
    f"{WARNINGS_AS_ERRORS} cargo check --workspace --all-targets -q && "
    f"{WARNINGS_AS_ERRORS} cargo check --release --workspace --all-targets -q && "
    f"{WARNINGS_AS_ERRORS} cargo build --release -q --bins && "
    f"{WARNINGS_AS_ERRORS} cargo test --release -q --workspace --no-run"
)
# Measurements compare two revisions, so their historical base is built without
# today's warning policy.
MEASURE_BUILD = "cargo build --release -q --bins && cargo test --release -q --workspace --no-run"
# The shipped build (Cargo.toml `[profile.dist]`): what the creep run on main measures. Not a gate step: three minutes cold.
DIST_BUILD = "cargo build --profile dist -q --bins"


# Commands. Each runs under bash in the repo root with CARGO_TARGET_DIR set.
def commands(p: Plan, cfg: dict, pkgs: dict[str, dict], skip_bins: frozenset[str] = frozenset(), skip_py: tuple[str, ...] = ()) -> dict[str, str]:
    scope = "--workspace" if p.packages is None else " ".join(f"-p {n}" for n in p.packages)
    # The root crate has no library: a diff that selects it alone (a bench tool) has no lib or doc tests to run.
    # (with another crate that has one, the root crate stays in the scope: it turns on the features the others need)
    libs = scope if p.packages is None or any((ROOT / pkgs[n]["dir"] / "src/lib.rs").exists() for n in p.packages) else ""
    cargo = "cargo test --release -q --no-fail-fast"
    split = cfg["split"]
    whole = set(cfg["whole"].values()) | set(cfg["exclusive"].values())
    skips = " ".join(f"--skip {s['filter']}" for s in split)
    cheap_bins = " ".join(f"--test {t}" for t in root_tests() if t != "timing" and t not in whole and t not in skip_bins)
    selected = set(p.packages or pkgs)
    ct = " ".join(f"-p {n} --test {t}" for n, t in crate_tests(pkgs) if n in selected)
    bench = (
        'out=$(uv run -q --project tools python tools/bench/bench.py); echo "$out" | tail -3; '
        'echo "$out" | tail -1 | grep -q " 0 problems"'
    )
    steps = {
        "build": BUILD,
        "lib": f"{cargo} {libs} --lib" if libs else "true",
        "doc": f"{cargo} {libs} --doc" if libs else "true",
        "integration": f"{cargo} {cheap_bins} -- {skips} --skip test_every_program_under_tests_run_prints_its_out",
        "crate-tests": f"{cargo} {ct}" if ct else "true",
        # Every scan runs, and any failing fails the step: the first red must not hide the others.
        "scans": "rc=0; "
        + "; ".join(f"{cargo} -p {one['package']} " + (f"--lib -- {one['lib']}" if "lib" in one else f"--test {one['test']}") + " || rc=1" for one in cfg["scan"] if "package" in one)
        + "; exit $rc",
        "bench": bench,
        "torture": "timeout 600 uv run -q --project tools python tools/torture/torture.py --gate --work $CARGO_TARGET_DIR/torture-work",
        # Enforced or not (tiers.toml [fmt]), the check runs and lists the files. Unenforced, "files would change" (exit 1) passes
        # and anything else (2 or more: no toolchain, a failed build, a crash) still fails the step.
        "fmt": "tools/fmt.sh --check"
        if cfg["fmt"]["enforced"]
        else "tools/fmt.sh --check; rc=$?; [ $rc -le 1 ] || exit $rc; [ $rc = 0 ] || echo 'fmt: not enforced yet (tiers.toml [fmt])'",
        "rfmt-post": 'cargo test --release -q --manifest-path tools/rfmt-post/Cargo.toml --target-dir "${CARGO_TARGET_DIR:-target}/rfmt-post"',
        "pytest": "uv run -q --project tools python -m pytest tools crates tests/*.py -q -p no:cacheprovider --ignore=tests/test_programs_compile.py --ignore=tests/test_loops.py" + "".join(f" --ignore={f}" for f in skip_py),
        "pytest-programs": "uv run -q --project tools python -m pytest tests/test_programs_compile.py tests/test_loops.py -q -p no:cacheprovider",
        "qcport": "[ -f ~/scratch/qcport-env.sh ] || { echo SKIPPED: no ~/scratch/qcport-env.sh; exit 77; }; . ~/scratch/qcport-env.sh && uv run -q --project tools python tools/qcport-run.py",
        "measure": "[ -f ~/scratch/qcport-env.sh ] && . ~/scratch/qcport-env.sh; python3 tools/measure.py check",
        "run": f"LLRM_RUN_ONLY='{' '.join(p.languages)}' {cargo} --test run -- test_every_program_under_tests_run_prints_its_out",
    }
    for s in split:
        steps[s["step"]] = f"{cargo} --test {s['bin']} -- {s['filter']}"
    for step, binary in {**cfg["whole"], **cfg["exclusive"]}.items():
        steps[step] = f"{cargo} --test {binary}"
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
    One whose LLRM_REQUIRE_* variable the caller set is never missing: its tests fail, as they always did."""
    environ = os.environ if environ is None else environ
    out = {}
    if not environ.get("GATE_ALLOW_MISSING"):
        return out
    for name, cap in load().get("capability", {}).items():
        if any(environ.get(v) for v in cap["require"]):
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


def python_tests_unusable(missing: dict[str, str], root: Path = ROOT) -> list[str]:
    """Python test files whose own REQUIRES names a capability this host lacks."""
    return sorted(f for f, needs in declared_requirements(root).items() if any(c in missing for c in needs))


def unusable(missing: dict[str, str]) -> tuple[list[str], frozenset[str]]:
    """(run languages, cheap test binaries) that need a capability this host lacks."""
    out = {k: [c for c in needs if c in missing] for k, needs in load().get("requires", {}).items()}
    langs = [k[5:] for k, hit in out.items() if hit and k.startswith("lang:")]
    return langs, frozenset(k[4:] for k, hit in out.items() if hit and k.startswith("bin:"))


def groups_of(p: Plan, skipped: dict[str, str]) -> dict[str, list[str]]:
    """The CI job groups that have a step to run: group -> its steps in the plan, minus the skipped."""
    return {g: run for g, members in load()["groups"].items() if (run := [s for s in members if s in p.steps and s not in skipped])}


def run_step(name: str, command: str, logs: Path, env: dict, check: tuple[int | None, bool] | None = None, unset: tuple[str, ...] = ()) -> tuple[str, int, float]:
    start = time.time()
    # debug-gate.env makes every tool a requirement; a capability this host lacks is not one.
    prelude = ". tools/debug-gate.env 2>/dev/null; " + "".join(f"unset {v}; " for v in unset) + "set -o pipefail; "
    with open(logs / f"{name}.log", "w") as log:
        code = subprocess.call(["bash", "-c", prelude + command], cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    if code == 0 and check and (why := incomplete((logs / f"{name}.log").read_text(), *check)):
        (logs / f"{name}.log").open("a").write(f"\nINCOMPLETE: {why}\n")
        code = 78
    if code not in (0, 77):
        # The next run of the step overwrites its log; the failure is evidence, kept.
        (logs / f"{name}.failed.log").write_text((logs / f"{name}.log").read_text())
    return name, code, time.time() - start


def execute(p: Plan, group: str | None = None) -> tuple[int, list[str]]:
    target = os.environ.get("CARGO_TARGET_DIR")
    if not target:
        sys.exit("gate: CARGO_TARGET_DIR is not set")
    # The run test compares the tree before and after; a tool writing a .pyc into it meanwhile is not a leak.
    env = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1", "LLRM_BIN": os.environ.get("LLRM_BIN", f"{target}/release")}
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
        members = [] if group == "build" else load()["groups"][group]  # "build": the build alone, for the job that makes what the others share
        p = Plan(p.tier, p.reason, [s for s in p.steps if s == "build" or s in members], p.packages, p.languages)
    start = time.time()
    results: dict[str, tuple[int, float]] = {}

    def report(name: str, code: int, took: float) -> None:
        results[name] = (code, took)
        verdict = "ok" if code == 0 else "SKIPPED" if code == 77 else "INCOMPLETE" if code == 78 else "FAIL"
        print(f"[time] {name} {verdict} {took:.0f}s", flush=True)
        if code not in (0, 77):
            tail = (logs / f"{name}.log").read_text().splitlines()[-30:]
            print("\n".join("    " + line for line in tail), flush=True)

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
            print(f"GATE {p.tier} FAIL: build")
            return 1, ["build"]
    alone = [s for s in p.steps if s in load()["exclusive"] or s in SERIAL_STEPS]
    rest = [s for s in p.steps if s != "build" and s not in alone]
    jobs = int(os.environ.get("JOBS", "4"))
    with ThreadPoolExecutor(jobs) as pool:
        # The longest steps start first.
        order = sorted(rest, key=lambda s: s not in ("run", "identity", "qcport", "measure", "pytest-programs", "turbo", "bench"))
        for future in as_completed([pool.submit(run_step, s, cmds[s], logs, env, checks.get(s), unset) for s in order]):
            report(*future.result())
    for name in alone:
        report(*run_step(name, cmds[name], logs, env, checks.get(name), unset))
    failed = [n for n, (c, _) in results.items() if c not in (0, 77)]
    skipped = [n for n, (c, _) in results.items() if c == 77]
    cut = [n for n in failed if results[n][0] == 78]
    print(f"GATE {p.tier} {'FAIL: ' + ' '.join(failed) if failed else 'PASS'}{' (INCOMPLETE: ' + ' '.join(cut) + ')' if cut else ''} in {time.time() - start:.0f}s" + (f" (skipped: {' '.join(skipped)})" if skipped else ""))
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("command", choices=["plan", "run", "bisect", "main"])
    ap.add_argument("rest", nargs="*")
    ap.add_argument("--base", default="origin/main")
    ap.add_argument("--files", nargs="*")
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--group")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--tier", default="auto", choices=["auto", "fast", "full"])
    ap.add_argument("--steps", nargs="+", metavar="STEP", help="only these steps (the plan is otherwise as planned); `build measure`")
    args = ap.parse_args()
    if args.command == "main":
        return watch_main(args.force)
    if args.command == "bisect":
        return bisect(args.rest[0], args.rest[1], args.rest[2:])
    p = plan(args.files if args.files is not None else changed_files(args.base), args.tier)
    if args.steps:
        p = restricted(p, args.steps, set(commands(p, load(), packages())) | set(load()["exclusive"]))
    skipped = skipped_steps(p.steps, missing_capabilities(), p.languages)
    if args.json:
        print(json.dumps({"tier": p.tier, "steps": p.steps, "groups": groups_of(p, skipped), "skipped": skipped}))
        return 0
    print(f"tier {p.tier}: {p.reason}")
    for step, why in skipped.items():
        print(f"SKIPPED {step}: {why}")
    print("steps:", " ".join(p.steps), "| run languages:", " ".join(p.languages), "| crates:", "all" if p.packages is None else f"{len(p.packages)}")
    if args.command == "plan" or p.tier == "none":
        return 0
    return execute(p, args.group)[0]


if __name__ == "__main__":
    sys.exit(main())
