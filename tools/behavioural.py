#!/usr/bin/env python3
"""The behavioural tier: after a merge that touches the register allocator, the coalescer, liveness, the selector, the peephole or the frame, build the bins once and run
the call-shaped tests/run programs (the regparm3_matrix family and the programs that keep values across calls), and say so within minutes when one is wrong.

    tools/behavioural.py [--tip SHA] [--watch SECONDS]     one check of origin/feat/parity (or SHA), or a check every SECONDS after a fetch

Not a gate and not before a merge: the hourly sweep stays the official line (the 23:00 sweep of 2026-10-23 found #4152's wrong output 18 minutes after it landed; this
finds it after the build and a few seconds of runs). A red line is `BEHAVIOURAL RED: <merge> <program>`, appended to the log (BEHAVIOURAL_LOG, default
~/scratch/coordwork/behavioural.log) and printed; the coordinator routes the owner, fix forward or revert.
"""
from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = "origin/feat/parity"
LOG = Path(os.environ.get("BEHAVIOURAL_LOG") or Path.home() / "scratch/coordwork/behavioural.log")
STATE = Path(os.environ.get("BEHAVIOURAL_STATE") or Path.home() / "scratch/coordwork/behavioural.state")
WORKTREE = Path(os.environ.get("BEHAVIOURAL_WORKTREE") or Path.home() / "scratch/wt-behavioural")
TARGET = Path(os.environ.get("BEHAVIOURAL_TARGET") or Path.home() / "scratch/tgt-behavioural")

# What turns a value across a call wrong: any backend source (the coalescer, the allocator, the machine scheduler, liveness, the selector, the peephole, the frame, the
# two-address pass), and the target's selection rules. Not the DAG path under selectiondag/ (the default compile does not use it), and not tests. The first rule here
# named fixed paths and missed machinesched.rs and flow.rs: #4146 landed red with a tier that read "no merge touched the paths".
TARGET_RULES = re.compile(r"^crates/target/[^/]+/src/isel/")

# tests/run/c programs (a stem without a .out is skipped by discovery): the matrix family and the ones that keep a value, a frame or a register across a call
PROGRAMS = (
    "regparm3_matrix_22", "regparm3_matrix_31", "regparm3_matrix_4", "call_keeps", "double_across_a_call_result", "far_pointer_call", "far_return", "float_return",
    "frame_address_stack_args", "fsgs_call", "i64_param_home", "ia16_abi", "inlined_calls_keep_stores_through_far_alias", "inlined_tail_call_keeps_frame_stores",
    "long_double_arguments", "old_style_extra_args", "pair_return", "register_args_truncate", "spill_store_order", "sysv_abi", "x87_arguments",
    # the other frontends and runtimes #4146 broke (10 of 1014 at a1d904596): a memfuncs C program, QB programs, Nib examples, grep in BASIC
    "memfuncs_runtime", "bounds", "range_power", "channels", "level", "grep",
)


def touches(files: list[str]) -> bool:
    """Whether a merge changes code the call-shaped programs depend on: backend source or a target's selection rule, not a doc, a test, or the DAG path."""
    backend = lambda f: f.startswith(("crates/backend/", "crates/codegen/", "crates/driver/llrm-driver/")) and f.endswith(".rs") and "/selectiondag" not in f  # noqa: E731
    rule = lambda f: bool(TARGET_RULES.match(f)) and f.endswith((".rs", ".peep"))  # noqa: E731
    return any((backend(f) or rule(f)) and "_tests" not in f and "/tests/" not in f for f in files)


def merges(old: str, new: str, cwd: Path = ROOT) -> list[tuple[str, str, bool]]:
    """(sha, subject, touches the tier's paths) of the first-parent commits in old..new, oldest first."""
    out = subprocess.run(["git", "log", "--first-parent", "--reverse", "--name-only", "--format=@@%H\t%s", f"{old}..{new}"], cwd=cwd, capture_output=True, text=True, check=True).stdout
    found, current = [], None
    for line in out.splitlines():
        if line.startswith("@@"):
            sha, _, subject = line[2:].partition("\t")
            current = [sha, subject, []]
            found.append(current)
        elif line and current is not None:
            current[2].append(line)
    return [(sha, subject, touches(files)) for sha, subject, files in found]


def red_programs(output: str) -> list[str]:
    """The programs `run_tests.py` printed a FAIL for: `FAIL  c/NAME.c [...]: ...`."""
    return sorted({m.group(1) for m in re.finditer(r"^FAIL\s+\S*?/?([\w.]+?)\.(?:c|bas|nib)\b", output, re.M)})


def red_lines(suspects: list[tuple[str, str, bool]], programs: list[str]) -> list[str]:
    """One `BEHAVIOURAL RED: <merge> <program>` per red program, naming the merges of the range that touch the paths (the culprit is among them; a range of one is it)."""
    named = [(sha, subject) for sha, subject, touching in suspects if touching] or [(sha, subject) for sha, subject, _ in suspects]
    who = " | ".join(f"{sha[:9]} {subject}" for sha, subject in named) or "?"
    return [f"BEHAVIOURAL RED: {who} {program}" for program in programs]


def build_and_run(tip: str) -> str:
    """The output of the programs on a release build of `tip` (built in its own worktree and target)."""
    if not WORKTREE.exists():
        subprocess.run(["git", "worktree", "add", "-q", "--detach", str(WORKTREE), tip], cwd=ROOT, check=True)
    subprocess.run(["git", "checkout", "-q", "--detach", tip], cwd=WORKTREE, check=True)
    env = {k: v for k, v in os.environ.items() if k != "RUSTFLAGS"} | {"CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(TARGET)}
    built = subprocess.run(["nice", "-n", "10", str(Path.home() / ".local/bin/buildslot"), "cargo", "build", "--release", "-q", "--bins"], cwd=WORKTREE, env=env, capture_output=True, text=True)
    if built.returncode:
        return f"FAIL  build: {built.stderr.strip()[-200:]}"
    binaries = TARGET / "release"
    for one in binaries.iterdir():
        if one.is_file():
            one.touch()  # the runners refuse a binary older than its sources
    run_env = {**env, "LLRM_BIN": str(binaries), "DOSBOX_BIN": str(binaries / "dosbox-x")}
    done = subprocess.run([sys.executable, "tools/dosbatch/run_tests.py", *PROGRAMS], cwd=WORKTREE, env=run_env, capture_output=True, text=True, timeout=1800)
    return done.stdout + done.stderr


def check(tip: str, last: str | None, build=build_and_run, log: Path | None = None) -> list[str]:
    """The red lines of one check: nothing when no merge since `last` touches the paths, else the programs wrong on `tip`."""
    suspects = merges(last, tip) if last else [(tip, "", True)]
    if not any(touching for _, _, touching in suspects):
        return []
    lines = red_lines(suspects, red_programs(build(tip)))
    if log and lines:
        log.parent.mkdir(parents=True, exist_ok=True)
        with open(log, "a") as handle:
            handle.write("\n".join(f"{time.strftime('%F %T')} {line}" for line in lines) + "\n")
    return lines


def once(tip_ref: str | None) -> int:
    subprocess.run(["git", "fetch", "-q", "origin", "feat/parity"], cwd=ROOT, check=True)
    tip = subprocess.run(["git", "rev-parse", tip_ref or BASE], cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    last = STATE.read_text().strip() if STATE.exists() else None
    if last == tip:
        return 0
    lines = check(tip, last, log=LOG)
    STATE.parent.mkdir(parents=True, exist_ok=True)
    STATE.write_text(tip + "\n")
    print("\n".join(lines) if lines else f"behavioural: {tip[:9]} ok (or no merge touched the paths since {(last or '-')[:9]})", flush=True)
    return 1 if lines else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--tip")
    parser.add_argument("--watch", type=int, metavar="SECONDS")
    args = parser.parse_args()
    if not args.watch:
        return once(args.tip)
    while True:
        once(args.tip)
        time.sleep(args.watch)


if __name__ == "__main__":
    sys.exit(main())
