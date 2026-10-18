#!/usr/bin/env python3
"""Compile every QCport module at -O1/-O2/-O3/-Os with QCport's flags on a feat/parity build, group the failures, file one issue per new kind, bisect it.

    tools/qcport_sweep.py --bin DIR [--issue] [--bisect GOOD]

A failure kind is the compiler's last line with numbers and block/label ids removed; `tools/qcport_sweep_known.toml` lists the kinds already filed (`match`,
`issue`). A kind in no entry is NEW: with --issue it is filed (title, repro, head sha, the modules and levels); with --bisect GOOD (a ref where it does not fail,
e.g. origin/main) the first-parent squashes GOOD..head are bisected with incremental release builds (one worktree and target, buildslot at nice 10) to the first bad one,
named in the issue. The report goes to stdout for the coordinator. Hourly: one dist target for the sweep, the bisect builds are the only other.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
import tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402

LEVELS = ("-O1", "-O2", "-O3", "-Os")
MODULES = ("host", "render", "model", "game", "sound", "ui")
KNOWN = HERE / "qcport_sweep_known.toml"


def kind(message: str) -> str:
    """The failure's kind: its last line without numbers, `name@3` function names and L1_125-style labels."""
    lines = message.strip().splitlines()
    for i, line in enumerate(lines):
        if "panicked at" in line and i + 1 < len(lines):  # the message under a panic, not rustc's trailing backtrace note
            where = re.sub(r"^.*panicked at ", "", line).rstrip(":")
            return re.sub(r"\d+", "N", f"panic {where}: {lines[i + 1].strip()}")[:160]
    last = next((line for line in reversed(lines) if line.strip() and not line.startswith("note: run with")), "no message")
    last = re.sub(r"@?\w+@\d+:?", "F", last)
    last = re.sub(r"\bL\d+_\d+\b", "L", last)
    last = re.sub(r"\(\s*[\d, ]+\)", "(...)", last)
    return re.sub(r"\d+", "N", last)[:160]


def sweep(compiler: Path, qcport: Path, include: Path, levels=LEVELS, jobs: int = 8) -> list[tuple[str, str, str]]:
    """(module, level, kind) of every failed compile."""
    flags = [f for part in (*MODULES, "qgl") for f in ("-I", str(qcport / part))] + ["-I", str(include)]
    todo = [(s, level) for part in MODULES for s in sorted((qcport / part).glob("*.c")) for level in levels]

    def one(job):
        source, level = job
        done = subprocess.run([str(compiler), level, *flags, str(source), "-o", os.devnull], capture_output=True, text=True)
        return None if done.returncode == 0 else (source.stem, level, kind(done.stderr or done.stdout))

    with ThreadPoolExecutor(jobs) as pool:
        return [r for r in pool.map(one, todo) if r]


ORACLES = ("DATAFLOW", "FACTS", "CLOBBERED", "COALESCE", "REPLAY", "INTERN", "SKIPPED", "STALE")
# Front ends and targets QCport does not cover: the Nib and BASIC front ends at both machines, with every oracle on (the smoke's `programs` step, here per level).
EXTRA = [(tool, ext, machine, level) for tool, ext in (("llrm-nib", "nib"), ("llrm-qb", "bas")) for machine in ("-m16", "-m32") for level in LEVELS]


def sweep_extra(bins: Path, jobs: int = 8) -> list[tuple[str, str, str]]:
    """(program, 'machine level', kind) of every failed compile of bench/bintree in Nib and BASIC at both machines and all levels, oracles on."""
    env = {**os.environ, **{f"LLRM_CHECK_{name}": "1" for name in ORACLES}}

    def one(job):
        tool, ext, machine, level = job
        done = subprocess.run([str(bins / tool), str(ROOT / f"bench/bintree/bintree.{ext}"), level, machine, "-o", os.devnull], capture_output=True, text=True, env=env)
        return None if done.returncode == 0 else (f"bintree.{ext}", f"{machine} {level}", kind(done.stderr or done.stdout))

    with ThreadPoolExecutor(jobs) as pool:
        return [r for r in pool.map(one, EXTRA) if r]


def grouped(failures: list[tuple[str, str, str]]) -> dict[str, list[tuple[str, str]]]:
    out: dict[str, list[tuple[str, str]]] = {}
    for module, level, why in failures:
        out.setdefault(why, []).append((module, level))
    return out


def new_kinds(groups: dict, known: list[dict]) -> dict:
    """The kinds no `match` of the known file covers."""
    return {why: where for why, where in groups.items() if not any(entry["match"] in why for entry in known)}


def first_bad(commits: list[str], is_bad) -> str:
    """The first bad commit of `commits` (old to new, the last one bad): binary search, `is_bad(commit)` builds and tests it. Assumes a single flip."""
    lo, hi = 0, len(commits) - 1  # hi is bad
    while lo < hi:
        mid = (lo + hi) // 2
        if is_bad(commits[mid]):
            hi = mid
        else:
            lo = mid + 1
    return commits[hi]


def bisect_one(good: str, repro: list[str], build) -> tuple[str, str]:
    """(sha, subject) of the first first-parent squash after `good` on which `repro` (a compile of the failing module) fails; `build(sha)` returns the llrm-c path."""
    shas = subprocess.run(["git", "log", "--first-parent", "--reverse", "--format=%H", f"{good}..origin/feat/parity"], cwd=ROOT, capture_output=True, text=True).stdout.split()
    bad = lambda sha: subprocess.run([str(build(sha)), *repro], capture_output=True).returncode != 0  # noqa: E731
    found = first_bad(shas, bad)
    subject = subprocess.run(["git", "log", "-1", "--format=%s", found], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    return found, subject


def worktree_build(sha: str) -> Path:
    """llrm-c at `sha`: one worktree, one target, incremental release builds under buildslot at nice 10."""
    tree = Path.home() / "scratch/qcport-sweep-tree"
    if not tree.exists():
        subprocess.run(["git", "worktree", "add", "-q", "--detach", str(tree), sha], cwd=ROOT, check=True)
    subprocess.run(["git", "checkout", "-q", "--detach", sha], cwd=tree, check=True)
    env = {**os.environ, "CARGO_TARGET_DIR": str(Path.home() / "scratch/qcport-sweep-target")}
    env.pop("RUSTFLAGS", None)
    subprocess.run(["nice", "-n", "10", str(Path.home() / ".local/bin/buildslot"), "cargo", "build", "--release", "-q", "--bin", "llrm-c"], cwd=tree, env=env, check=True)
    return Path(env["CARGO_TARGET_DIR"]) / "release/llrm-c"


def issue_text(why: str, where: list[tuple[str, str]], head: str, first: tuple[str, str] | None) -> tuple[str, str]:
    module, level = where[0]
    title = f"qcport sweep: {why[:90]} ({module} {level}{', ...' if len(where) > 1 else ''})"
    mods = ", ".join(sorted({f'{m} {l}' for m, l in where})[:12])
    body = (
        f"TL;DR: compiling QCport's {module}.c at {level} fails on feat/parity {head[:9]}: `{why}`.\n\n"
        f"Repro: `llrm-c {level} -I $QCPORT/{{host,render,model,game,sound,ui,qgl}} -I $QCPORT_INC $QCPORT/<dir>/{module}.c -o /dev/null` "
        f"(QCPORT=~/scratch/qcport/src, QCPORT_INC=~/scratch/qctc/inc).\n\nFails: {mods}{' ...' if len(where) > 12 else ''}.\n"
        + (f"\nFirst bad squash (first-parent bisection): {first[0][:9]} {first[1]}\n" if first else "")
    )
    return title, body


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bin", type=Path, default=None)
    parser.add_argument("--issue", action="store_true", help="file one issue per new failure kind")
    parser.add_argument("--bisect", metavar="GOOD", help="bisect each new kind from this good ref to the head")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "8")))
    args = parser.parse_args()
    qcport = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src")).expanduser()
    include = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc")).expanduser()
    compiler = (args.bin or llrmbin.bin_dir()) / "llrm-c"
    head = subprocess.run(["git", "rev-parse", "origin/feat/parity"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    failures = sweep(compiler, qcport, include, jobs=args.jobs) + sweep_extra(compiler.parent, jobs=args.jobs)
    groups = grouped(failures)
    known = tomllib.loads(KNOWN.read_text()).get("known", []) if KNOWN.exists() else []
    fresh = new_kinds(groups, known)
    modules = len(list((qcport / "render").glob("*.c"))) + sum(len(list((qcport / p).glob("*.c"))) for p in MODULES if p != "render")
    print(f"QCport sweep of feat/parity {head[:9]}: {modules} modules x {len(LEVELS)} levels + {len(EXTRA)} front-end compiles, {len(failures)} failed compiles, {len(groups)} kinds, {len(fresh)} new")
    for why, where in groups.items():
        entry = next((e for e in known if e["match"] in why), None)
        print(f"  {'KNOWN #' + str(entry['issue']) if entry else 'NEW'}: {why} -- {', '.join(sorted({f'{m} {l}' for m, l in where})[:8])}")
    for why, where in fresh.items():
        first = None
        if args.bisect:
            module, level = where[0]
            src = next((qcport / p / f"{module}.c" for p in MODULES if (qcport / p / f"{module}.c").exists()), None)
            flags = [level] + [f for part in (*MODULES, "qgl") for f in ("-I", str(qcport / part))] + ["-I", str(include), str(src), "-o", os.devnull]
            first = bisect_one(args.bisect, flags, worktree_build)
            print(f"  first bad squash of '{why[:60]}': {first[0][:9]} {first[1]}")
        if args.issue:
            title, body = issue_text(why, where, head, first)
            done = subprocess.run(["gh", "issue", "create", "--title", title, "--body", body], capture_output=True, text=True)
            print(f"  filed: {done.stdout.strip() or done.stderr.strip()}")
    return 1 if fresh else 0


if __name__ == "__main__":
    sys.exit(main())
