#!/usr/bin/env python3
"""answer_smoke.py [--bin DIR] [--work DIR] [PROGRAM...]: do the programs print what they should?

Each PROGRAM (default crc, nbody, NBODY_SINGLE, NBODY_FIXED and JPEG_FIXED) is a bench/NAME/NAME.bas with its expected output NAME.out.
It is compiled with its header's flags (-O2) by DIR's llrm-qb (default $LLRM_BIN or this tree's) for both targets (m16 real mode, m32 under DOS/32A), run in
DOSBox, and its output compared line by line (blanks and trailing spaces apart). Prints the binary's md5 and one line per run; the exit
code is the number of wrong answers, builds that failed and runs that did not end.

Not the bench: no counters, no emulator. A compiler that miscompiles `crc` or `nbody` prints another number, whatever it counts."""

from __future__ import annotations

import argparse
import hashlib
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent / "dosbatch"))

import dosbatch  # noqa: E402
import qbbuild  # noqa: E402
import run_tests  # noqa: E402

DEFAULT = ("crc", "nbody", "nbody_single", "nbody_fixed", "jpeg_fixed")
BUDGET_MS = 120_000
BENCH = dosbatch.ROOT / "bench"


def same(got: str, want: str) -> bool:
    """The printed text and the expected, as lines: BASIC's PRINT of a number leads with a blank."""
    return [one.strip() for one in run_tests.lines(got)] == [one.strip() for one in run_tests.lines(want)]


def check(
    names: tuple[str, ...],
    run: Callable[[str, str], tuple[str, str]],
    bench: Path = BENCH,
) -> list[str]:
    """One line per (program, target); `run(source, target)` is (status, output). A line starting with ok is a right answer."""
    out = []
    for name in names:
        want = (bench / name / f"{name}.out").read_text(errors="replace")
        for target in qbbuild.TARGETS:
            label = f"{name} m{dosbatch.m_flag(target)[2:]}"
            status, output = run(str(bench / name / f"{name}.bas"), target)
            if status != "ok":
                out.append(f"{label}: {status}")
            elif same(output, want):
                out.append(f"ok {label}")
            else:
                out.append(f"WRONG {label}: want {run_tests.lines(want)[:2]}, got {run_tests.lines(output)[:2]}")
    return out


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("programs", nargs="*", default=list(DEFAULT))
    parser.add_argument("--work", type=Path)
    args = parser.parse_args(argv)
    binary = dosbatch.BIN / "llrm-qb"
    print(f"llrm-qb {hashlib.md5(binary.read_bytes()).hexdigest()[:12]} ({dosbatch.BIN})", flush=True)
    with tempfile.TemporaryDirectory() as scratch:
        work = args.work or Path(scratch)
        objects = qbbuild.runtimes(work / "runtime")
        built: dict[str, dict] = {}

        def run(source: str, target: str) -> tuple[str, str]:
            name = Path(source).stem
            settings = run_tests.header(Path(source))
            if name not in built:
                flags = tuple(settings["flags"].split("|")[0].split()) if "flags" in settings else ("-O2",)
                built[name] = qbbuild.build(Path(source), work / name, objects, flags, tolerant=True)
            one = built[name][target]
            if isinstance(one, str):
                return f"did not build: {one}", ""
            data = tuple(Path(source).parent / one for one in settings.get("data", "").split())  # a program that reads a file: its `' data:` header
            ran = qbbuild.run(one, work / name / f"run-{dosbatch.m_flag(target)}", data, budget_ms=BUDGET_MS)
            return ran.status, ran.output.decode("latin1")

        lines = check(tuple(args.programs), run)
    print("\n".join(lines))
    wrong = sum(not one.startswith("ok ") for one in lines)
    print(f"{wrong} wrong of {len(lines)}")
    return wrong


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
