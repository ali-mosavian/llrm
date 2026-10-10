#!/usr/bin/env python3
"""What it costs llrm-c to compile: user-space instructions per file at -O1, -O2 and -Os.

    tools/compile-cost.py [--noise] [llrm-c]     print the measurement (tools/measure.py compares it with the merge-base's)

Files: the 66 vsgcc programs (bench/ and the x_ kernels, wrapped as vsgcc compiles them) and, when QCPORT and QCPORT_INC
are set, QCport's modules. Instructions come from `perf stat -e instructions:u`, one process per compile: a count of
work done, not of time taken, so host load does not move it. Exit 77 if perf cannot count (no permission, a VM with no
counter), never a pass on a zero. `--noise` measures twice and prints the spread: the same build differs by 0.0001 in
geomean and 0.0047 at worst over 393 files (2026-10-08), from which the tolerances in tiers.toml [measure] come.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
VSGCC = next((ROOT / "crates/target").glob("*/vsgcc"))  # the target the comparison is for owns its programs and flags
LEVELS = ("-O1", "-O2", "-Os")
SCRATCH = os.environ.get("CARGO_TARGET_DIR")  # never /tmp
MODULES = ("host", "render", "model", "game", "sound", "ui")

sys.path.insert(0, str(HERE))
sys.path.insert(0, str(VSGCC))
import levels_time  # noqa: E402
import llrmbin  # noqa: E402
import programs  # noqa: E402
import wrap  # noqa: E402


class NoCounter(Exception):
    pass


def instructions(command: list[str]) -> int:
    """User-space instructions `command` retires, children included. The compiler must succeed."""
    with tempfile.NamedTemporaryFile("r", dir=SCRATCH) as out:
        env = {k: v for k, v in os.environ.items() if not k.startswith("LLRM_") or k == "LLRM_BIN"}  # LLRM_VERIFY adds checking work
        done = subprocess.run([*levels_time.UNRANDOMIZED, "perf", "stat", "-x,", "-e", "instructions:u", "-o", out.name, "--", *command], capture_output=True, text=True, env=env)
        counts = [line.split(",") for line in out.read().splitlines() if "instructions" in line]
    if done.returncode:
        raise SystemExit(f"{' '.join(command[-3:])}: exit {done.returncode}\n{done.stderr[-400:]}")
    if not counts or not counts[0][0].isdigit() or int(counts[0][0]) == 0:
        raise NoCounter(f"perf counted {counts or 'nothing'}")
    return int(counts[0][0])


def files(work: Path) -> dict[str, tuple[Path, list[str]]]:
    """Name -> (source, extra flags): the 66 programs, wrapped as vsgcc/build.sh does, and QCport when it is available."""
    found: dict[str, tuple[Path, list[str]]] = {}
    sources = programs.sources()
    for name, source in sources.items():
        wrapped = work / f"{name}.c"
        wrapped.write_text(wrap.wrapped(name, source.read_text()))
        found[name] = (wrapped, programs.LLRM_FLAGS)
    if (qcport := os.environ.get("QCPORT")) and (headers := os.environ.get("QCPORT_INC")):
        base = Path(qcport).expanduser()
        include = [flag for d in (*MODULES, "qgl") for flag in ("-I", str(base / d))] + ["-I", str(Path(headers).expanduser())]
        for d in MODULES:
            for source in sorted((base / d).glob("*.c")):
                found[f"qcport/{source.stem}"] = (source, include)
    return found


def measure(compiler: Path, jobs: int) -> dict[str, int]:
    """Instructions per `file level`."""
    with tempfile.TemporaryDirectory(dir=SCRATCH) as tmp:
        work = Path(tmp)
        todo = [(f"{name} {level}", [str(compiler), level, *flags, str(source), "-o", os.devnull]) for name, (source, flags) in files(work).items() for level in LEVELS]
        with ThreadPoolExecutor(jobs) as pool:
            return dict(zip((k for k, _ in todo), pool.map(instructions, (c for _, c in todo))))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--noise", action="store_true", help="measure twice, print the spread")
    parser.add_argument("--jobs", type=int, default=int(os.environ.get("JOBS", "4")))
    parser.add_argument("compiler", nargs="?", type=Path)
    args = parser.parse_args()
    args.compiler = args.compiler or llrmbin.bin_dir() / "llrm-c"
    try:
        now = measure(args.compiler, args.jobs)
        if args.noise:
            again = measure(args.compiler, args.jobs)
            spread = sorted(abs(again[k] / v - 1) for k, v in now.items())
            print(f"files {len(now)}, spread median {spread[len(spread) // 2]:.6f}, p99 {spread[int(len(spread) * .99)]:.6f}, max {spread[-1]:.6f}")
            return 0
    except NoCounter as why:
        print(f"SKIPPED: instruction counter unavailable ({why})")
        return 77
    print(json.dumps(dict(sorted(now.items()))))
    return 0


if __name__ == "__main__":
    sys.exit(main())
