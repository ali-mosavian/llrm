#!/usr/bin/env python3
"""Code bytes of every run_tests program at -Os, two builds, one selector: which variants grow, which shrink.

    tools/pairs/sizesweep.py BEFORE_DIST AFTER_DIST [--isel old|dag] [--level -Os]

Prints `name [before, after]` for each variant whose size differs, then `TOTAL [before, after]` over the variants both builds compiled, then the
summary: variants grown and shrunk, bytes each way. `summarize` reads that text back; the TOTAL line is no variant.
"""
from __future__ import annotations

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


def sweep(before: str, after: str, isel: str, level: str) -> list[str]:
    sys.path[:0] = [str(Path(__file__).resolve().parents[1]), str(Path(__file__).resolve().parents[1] / "dosbatch")]
    import run_tests
    import sizes

    out, totals = [], [0, 0]
    scratch = Path(tempfile.mkdtemp(prefix="sizesweep-"))  # one per run: a fixed /tmp path was shared by concurrent sweeps
    try:
        return _sweep(run_tests, sizes, scratch, before, after, isel, level, out, totals)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)


def _sweep(run_tests, sizes, scratch: Path, before: str, after: str, isel: str, level: str, out: list[str], totals: list[int]) -> list[str]:
    for program in run_tests.discover([]):
        tool = {".bas": "llrm-qb", ".nib": "llrm-nib", ".c": "llrm-c"}[program.source.suffix]
        measured = []
        for dist in (before, after):
            obj = scratch / "sizesweep.obj"
            obj.unlink(missing_ok=True)
            dialect = ["--dialect", program.dialect, "--runtime", program.dialect] if program.source.suffix == ".bas" else []
            flags = [flag for flag in program.flags if not flag.startswith("-O")]
            subprocess.run([f"{dist}/{tool}", str(program.source), *dialect, *flags, level, f"--isel={isel}", "-o", str(obj)], capture_output=True)
            measured.append(sizes.code_bytes(obj) if obj.exists() else None)
        if None in measured:
            continue
        totals = [totals[0] + measured[0], totals[1] + measured[1]]
        if measured[0] != measured[1]:
            out.append(f"{program.source.parent.name}/{program.source.name} {measured}")
    return [*out, f"TOTAL {totals}"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("before")
    parser.add_argument("after")
    parser.add_argument("--isel", default="dag")
    parser.add_argument("--level", default="-Os")
    args = parser.parse_args()
    lines = sweep(args.before, args.after, args.isel, args.level)
    print("\n".join(lines))
    print(summarize("\n".join(lines)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
