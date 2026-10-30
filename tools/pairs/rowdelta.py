#!/usr/bin/env python3
"""The rows that moved between two pindiff tables (two builds, same level and corpus).

    rowdelta.py BEFORE.txt AFTER.txt     one line per row whose instructions or nopad bytes differ: `name: ins a->b (pin p), nopad a->b (pin q)`

Compare only tables from the same corpus and level: pindiff's default corpus changed on 2026-10-27 and older tables do not match it.
"""
from __future__ import annotations

import re
import sys

ROW = re.compile(r"^(\S+)\s+(\d+)\s+(\d+)\s+[+-]\d+ \|\s+\d+\s+\d+\s+[+-]\d+ \|\s+(\d+)\s+(\d+)\s+[+-]\d+")


def parse(text: str) -> dict[str, tuple[int, int, int, int]]:
    """name -> (ins, pin ins, nopad bytes, pin nopad bytes)."""
    rows = {}
    for line in text.splitlines():
        found = ROW.match(line)
        if found:
            rows[found.group(1)] = tuple(int(found.group(i)) for i in (2, 3, 4, 5))
    return rows


def deltas(before: str, after: str) -> list[str]:
    old, new = parse(before), parse(after)
    return [
        f"{name}: ins {old[name][0]}->{new[name][0]} (pin {new[name][1]}), nopad {old[name][2]}->{new[name][2]} (pin {new[name][3]})"
        for name in sorted(new)
        if name in old and (old[name][0], old[name][2]) != (new[name][0], new[name][2])
    ]


if __name__ == "__main__":
    print("\n".join(deltas(open(sys.argv[1]).read(), open(sys.argv[2]).read())))
