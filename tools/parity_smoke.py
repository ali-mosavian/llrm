#!/usr/bin/env python3
"""The per-merge smoke set of the integration branch feat/parity: the most bugs caught per second, 10 s in all.

    tools/parity_smoke.py [--bin DIR]

The last line is `PARITY SMOKE PASS ...` or `PARITY SMOKE FAIL ...` with the seconds spent. A step that cannot run here (no QCport sources) is
named as skipped and is not a pass of that step. Steps, with what they were measured at (release build, 8 oracles on where it says):
  programs  bintree as BASIC, Nib and C at -m16 and -m32, every LLRM_CHECK_* oracle on        ~1.5 s  frontends, both machines, the oracles
  sc        QCport's render/sc.c at -m16 -O1, every oracle on                                  ~2.0 s  the C front end on real 16-bit code (the unsound memory summary)
  d_faces   d_faces.c -m32 -O2 run in unicorn, the reports equal gcc -O0's                      ~2.3 s  a miscompile of real code, executed
The budget is CPU seconds (a loaded host stretches the wall clock of the same work 2-3x; wall is printed). The set exceeds it when the steps' spent time does (the run fails) or when their declared costs do (a test fails): add a step only by
dropping or cheapening another. The oracles on d_faces (1.3-4.7 s with the host loaded) and the unit tests of a touched crate (building them is the cost) are not here.
"""
from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import llrmbin  # noqa: E402

BUDGET = 10.0
ORACLES = ("DATAFLOW", "FACTS", "CLOBBERED", "COALESCE", "REPLAY", "INTERN", "SKIPPED", "STALE")
PROGRAM = ROOT / "bench" / "bintree"
# d_faces.c as the scoreboard reads it (sha256 prefix), and the six reports `env.c` printed for gcc -O0: an llrm build must print the same.
DFACES_SHA = "1ab72e38cab4fbd2"
DFACES_REPORTS = [1502227429, 443461, -222045, 1788001, 271784, 714164]
KEYWORDS = "far huge near pascal cdecl __far _far __near _near"


def oracle_env() -> dict[str, str]:
    return {**os.environ, **{f"LLRM_CHECK_{name}": "1" for name in ORACLES}}


def compile_ok(command: list[str], env: dict[str, str]) -> str | None:
    """None where the compile succeeded; else its first line of complaint."""
    done = subprocess.run(command, capture_output=True, text=True, env=env)
    if done.returncode == 0:
        return None
    lines = [line for line in (done.stderr or done.stdout).splitlines() if line.strip()]
    return f"exit {done.returncode}: {lines[-1][:140] if lines else 'no message'}  [{' '.join(command[:3])}]"


def programs(bins: Path) -> tuple[str, str]:
    """('ok'|'fail'|'skipped', detail)"""
    jobs = []
    for tool, ext in (("llrm-qb", "bas"), ("llrm-nib", "nib"), ("llrm-c", "c")):
        for machine in ("-m16", "-m32"):
            extra = ["-mabi=sysv", "-march=i486"] if tool == "llrm-c" and machine == "-m32" else []
            jobs.append([str(bins / tool), str(PROGRAM / f"bintree.{ext}"), "-O2", machine, *extra, "-o", os.devnull])
    for command in jobs:
        if why := compile_ok(command, oracle_env()):
            return "fail", why
    return "ok", f"{len(jobs)} compiles"


def sc(bins: Path) -> tuple[str, str]:
    qcport = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src")).expanduser()
    include = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc")).expanduser()
    source = qcport / "render" / "sc.c"
    if not source.is_file() or not include.is_dir():
        return "skipped", f"no QCport sources ({qcport})"
    flags = [f for part in ("", "render", "game", "host", "qgl", "model", "sound", "ui") for f in ("-I", str(qcport / part if part else qcport))] + ["-I", str(include)]
    why = compile_ok([str(bins / "llrm-c"), "-O1", *flags, str(source), "-o", os.devnull], oracle_env())
    return ("fail", why) if why else ("ok", "sc.c -m16 -O1")


def d_faces(bins: Path) -> tuple[str, str]:
    src = Path(os.environ.get("DFACES_SRC", Path.home() / "scratch/qbq-llrm/src")).expanduser()
    file = src / "render" / "d_faces.c"
    if not file.is_file():
        return "skipped", f"no d_faces.c ({src})"
    if hashlib.sha256(file.read_bytes()).hexdigest()[:16] != DFACES_SHA:
        return "skipped", f"{file} is not the pinned d_faces.c ({DFACES_SHA}): the reference reports are its"
    sys.path.insert(0, str(ROOT / "crates/target/llrm-x86-m32/vsgcc/dfaces"))
    os.environ["LLRM_BIN"], os.environ["DFACES_SRC"] = str(bins), str(src)
    with tempfile.TemporaryDirectory() as work:
        os.environ["VSGCC_WORK"] = work
        try:
            import run  # noqa: PLC0415  the scoreboard's runner: loads unicorn, so only here
            got = run.run("llrm")["reports"]
        except (AssertionError, ModuleNotFoundError) as why:
            return "fail", f"d_faces did not run: {str(why)[-200:]}"
    return ("ok", "reports equal gcc -O0's") if got == DFACES_REPORTS else ("fail", f"d_faces reports {got}, gcc -O0 printed {DFACES_REPORTS}")


STEPS = [("programs", programs, 1.5), ("sc", sc, 2.0), ("d_faces", d_faces, 2.5)]  # (name, step, declared seconds)


def cpu() -> float:
    """CPU seconds of this process and its children: a loaded host (load 60 here) stretches the wall clock of the same work 2-3x and failed no merge by it."""
    t = os.times()
    return t.user + t.system + t.children_user + t.children_system


def run_steps(steps, bins: Path, clock=cpu, budget: float = BUDGET) -> tuple[bool, str]:
    """(passed, the last line). The clock is the test's, which waits on virtual time."""
    started, wall, parts, failed, skipped = clock(), time.monotonic(), [], [], []
    for name, step, _ in steps:
        began = clock()
        verdict, detail = step(bins)
        parts.append(f"{name} {verdict} {clock() - began:.1f} s")
        if verdict == "fail":
            failed.append(f"{name}: {detail}")
        elif verdict == "skipped":
            skipped.append(f"{name}: {detail}")
    spent = clock() - started
    if spent > budget:
        failed.append(f"over the budget: {spent:.1f} s > {budget:.0f} s")
    line = f"{' | '.join(parts)} | {spent:.1f} s CPU of {budget:.0f} s ({time.monotonic() - wall:.1f} s wall)" + (f" | skipped {'; '.join(skipped)}" if skipped else "")
    if failed:
        return False, f"PARITY SMOKE FAIL {'; '.join(failed)} | {line}"
    return True, f"PARITY SMOKE PASS {line}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--bin", type=Path, default=None, help="the llrm-c, llrm-qb and llrm-nib directory (default: LLRM_BIN, else the target dir's release)")
    args = parser.parse_args()
    ok, line = run_steps(STEPS, args.bin or llrmbin.bin_dir())
    print(line)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
