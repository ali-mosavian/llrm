#!/usr/bin/env python3
"""Code bytes of every run_tests program at -Os, two builds, one selector: which variants grow, which shrink.

    tools/pairs/sizesweep.py BEFORE_DIST AFTER_DIST --target m16|m32 [--isel old|dag] [--level -Os]

--target is required and names what is measured: every program is compiled with that `-m` flag forced, and the variants whose own flags name another target are left
out and counted in the header. run_tests.discover lists the default-target (m16) program and, for each C and Nib program that also runs flat, an `-m32` variant: a sweep
that took them all added 16-bit and 32-bit bytes into one TOTAL (650 and 411 of 1061 variants), and was cited as "m16 -Os".

Prints `name [before, after]` for each variant whose size differs, then `TOTAL [before, after]` over the variants both builds compiled, then the
summary: variants grown and shrunk, bytes each way. `summarize` reads that text back; the TOTAL line is no variant.
"""
from __future__ import annotations

import os
import argparse
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROW = re.compile(r"^(\S+) \[(\d+), (\d+)\]\s*$")


def summarize(text: str) -> str:
    """The summary of a sweep's printed rows. The `TOTAL` line is the sum of the rows above it, so it is not a row: counted as one it doubled the bytes."""
    rows, total = [], None
    for line in text.splitlines():
        found = ROW.match(line)
        if not found:
            continue
        name, before, after = found[1], int(found[2]), int(found[3])
        if name == "TOTAL":
            total = (before, after)
        else:
            rows.append((name, before, after))
    grown = [row for row in rows if row[2] > row[1]]
    shrunk = [row for row in rows if row[2] < row[1]]
    line = f"{len(grown)} variants grow by {sum(a - b for _, b, a in grown)} B, {len(shrunk)} shrink by {sum(b - a for _, b, a in shrunk)} B"
    if total:
        line += f"; total {total[0]} -> {total[1]} ({total[1] - total[0]:+d} B)"
    return line


TARGETS = {"m16": "-m16", "m32": "-m32"}
CC_ROWS = {"m16": "x86_16-wat", "m32": "x86_32-wat"}


def own_target(flags: list[str]) -> str:
    """The target a program's own flags name (`m16`, `m32`), else `m16`: the compilers' default target is the 16-bit DOS one ."""
    for flag in flags:
        if flag in ("-m16", "-m32"):
            return "m" + flag[2:]
    return "m16"


def require_target(command: list[str], target: str) -> None:
    """Refuses a command that does not compile at `target`: the last `-m16`/`-m32` of its flags must be the one forced, so a sweep labelled m16 never measures m32."""
    named = [flag for flag in command if flag in ("-m16", "-m32")]
    if not named or named[-1] != TARGETS[target]:
        raise SystemExit(f"sizesweep: labelled {target} but would compile at {named[-1] if named else 'the default target'}: {' '.join(command[:3])}")


def sweep(before: str, after: str, isel: str, level: str, target: str) -> list[str]:
    sys.path[:0] = [str(Path(__file__).resolve().parents[1]), str(Path(__file__).resolve().parents[1] / "dosbatch")]
    import run_tests
    import sizes

    out, totals = [], [0, 0]
    scratch = Path(tempfile.mkdtemp(prefix="sizesweep-"))  # one per run: a fixed /tmp path was shared by concurrent sweeps
    try:
        return _sweep(run_tests, sizes, scratch, before, after, isel, level, target, out, totals)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def _sweep(run_tests, sizes, scratch: Path, before: str, after: str, isel: str, level: str, target: str, out: list[str], totals: list[int]) -> list[str]:
    import llrmbin

    programs = [program for program in run_tests.discover([]) if own_target(program.flags) == target]
    skipped = len(run_tests.discover([])) - len(programs)
    header = [f"# target {target}: {len(programs)} programs compiled with {TARGETS[target]} forced, {skipped} variants of the other target left out; {level} --isel={isel}; cpu: the compiler's default (no -march)"]
    for program in programs:
        tool = {".bas": "llrm-qb", ".nib": "llrm-nib", ".c": "llrm-cc"}[program.source.suffix]
        measured = []
        for dist in (before, after):
            obj = scratch / "sizesweep.obj"
            obj.unlink(missing_ok=True)
            dialect = ["--dialect", program.dialect, "--runtime", program.dialect] if program.source.suffix == ".bas" else []
            flags = [flag for flag in program.flags if not flag.startswith("-O") and flag not in ("-m16", "-m32")]
            cc = tool == "llrm-cc"
            command = [f"{dist}/{tool}", str(program.source), *dialect, *flags, TARGETS[target], level, f"--isel={isel}", *(["-c"] if cc else []), "-o", str(obj)]
            require_target(command, target)
            # llrm-cc takes its target row from LLRM_CC_TARGET; it links without -c.
            env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": CC_ROWS[target]} if cc else None
            subprocess.run(command, capture_output=True, env=env)
            measured.append(sizes.code_bytes(obj) if obj.exists() else None)
        if None in measured:
            continue
        totals = [totals[0] + measured[0], totals[1] + measured[1]]
        if measured[0] != measured[1]:
            out.append(f"{program.source.parent.name}/{program.source.name} {measured}")
    return [*header, *out, f"TOTAL {totals}"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("before")
    parser.add_argument("after")
    parser.add_argument("--target", required=True, choices=sorted(TARGETS))
    parser.add_argument("--isel", default="dag")
    parser.add_argument("--level", default="-Os")
    args = parser.parse_args()
    lines = sweep(args.before, args.after, args.isel, args.level, args.target)
    print("\n".join(lines))
    print(summarize("\n".join(lines)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
