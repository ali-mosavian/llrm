#!/usr/bin/env python3
"""Delta-reduce a text file (C source, MIR/LLVM IR, Nib, BASIC) to the fewest lines for which a command still fails.

    tools/reduce.py FILE --cmd 'COMMAND {}' [--out FILE.reduced] [--unit lines|blocks]

COMMAND runs with `{}` replaced by the path of a candidate; exit status 0 means the candidate still shows the failure (is interesting), anything else
that it does not, which includes a candidate that no longer compiles or verifies: the command is where that is decided. The FILE itself must be interesting.
Reduction is ddmin on the lines (chunks of halving size, each removal kept if the command still exits 0, until no single line can go), then the same
on blank-line-separated blocks first with --unit blocks (functions of an IR file). Writes FILE.reduced (or --out) and prints the line counts.
"""
from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path


def interesting(command: str, lines: list[str], suffix: str, work: Path) -> bool:
    path = work / f"candidate{suffix}"
    path.write_text("".join(lines))
    return subprocess.run(command.replace("{}", str(path)), shell=True, capture_output=True).returncode == 0


def ddmin(items: list[str], keeps) -> list[str]:
    """The smallest sublist `keeps` accepts that this greedy halving finds: chunks of n/2, n/4 ... 1 are dropped while the rest still passes."""
    size = max(len(items) // 2, 1)
    while True:
        changed = False
        at = 0
        while at < len(items):
            trial = items[:at] + items[at + size:]
            if trial != items and keeps(trial):
                items, changed = trial, True
            else:
                at += size
        if size == 1 and not changed:
            return items
        size = max(size // 2, 1) if not (size == 1 and changed) else 1


def blocks(lines: list[str]) -> list[list[str]]:
    """Runs of lines separated by blank lines, each with its trailing blank."""
    out, cur = [], []
    for line in lines:
        cur.append(line)
        if not line.strip():
            out.append(cur)
            cur = []
    return out + ([cur] if cur else [])


def reduce(lines: list[str], keeps, unit: str = "lines") -> list[str]:
    """`lines` reduced: blocks first when asked, then single lines."""
    if unit == "blocks":
        kept = ddmin(blocks(lines), lambda bs: keeps([line for b in bs for line in b]))
        lines = [line for b in kept for line in b]
    return ddmin(lines, keeps)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("file", type=Path)
    parser.add_argument("--cmd", required=True, help="exit 0 = still failing; {} is the candidate's path")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--unit", choices=("lines", "blocks"), default="lines")
    args = parser.parse_args()
    lines = args.file.read_text().splitlines(keepends=True)
    with tempfile.TemporaryDirectory() as tmp:
        work = Path(tmp)
        keeps = lambda candidate: interesting(args.cmd, candidate, args.file.suffix, work)  # noqa: E731
        if not keeps(lines):
            print(f"{args.file} is not interesting: the command does not exit 0 on it", file=sys.stderr)
            return 2
        small = reduce(lines, keeps, args.unit)
    out = args.out or args.file.with_suffix(args.file.suffix + ".reduced")
    out.write_text("".join(small))
    print(f"{len(lines)} -> {len(small)} lines: {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
