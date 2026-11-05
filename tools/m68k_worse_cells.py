#!/usr/bin/env python3
"""Count the m68k matrix cells where ours is worse than gcc or clang, strictly (no tolerance).

Os: bytes. O2 and above: bytes over, minus the per-CPU allowance (B per clock saved, read from
the tsv header) times the clocks saved. A cell needs ours, gcc and clang rows to be comparable.
Usage: m68k_worse_cells.py m68k.tsv [-v]
"""
import re
import sys

RATES = re.compile(r"byte_rate: (.*?) B per clk")


def rates(header: str) -> dict[str, float]:
    found = RATES.search(header)
    if not found:
        raise SystemExit("no byte_rate in the header")
    table: dict[str, float] = {}
    for part in found.group(1).split(","):
        names, value = part.split()
        for name in names.split("/"):
            table[name if name.startswith("68") else "68" + name] = float(value)
    return table


def excess(level: str, rate: float, ours: tuple[int, int], other: tuple[int, int]) -> float:
    extra = ours[0] - other[0]
    return extra if level == "Os" else extra - rate * (other[1] - ours[1])


def cells(lines):
    header, rows = "", []
    for line in lines:
        if line.startswith("#"):
            header += line
        else:
            rows.append(line.rstrip("\n").split("\t"))
    table = rates(header)
    seen: dict[tuple[str, str, str], dict[str, tuple[int, int]]] = {}
    for row in rows[1:]:
        try:
            seen.setdefault((row[0], row[1], row[2]), {})[row[3]] = (int(row[4]), int(row[5]))
        except (ValueError, IndexError):
            continue
    for (target, prog, level), by in seen.items():
        if all(name in by for name in ("ours", "gcc", "clang")):
            rate = table[target.split("-")[1]]
            yield target, prog, level, max(excess(level, rate, by["ours"], by[c]) for c in ("gcc", "clang"))


def count(lines) -> tuple[int, int]:
    results = list(cells(lines))
    return sum(1 for *_, e in results if e > 0), len(results)


if __name__ == "__main__":
    if "-v" in sys.argv:
        for target, prog, level, e in sorted(cells(open(sys.argv[1])), key=lambda c: -c[3]):
            if e > 0:
                print(f"{target} {prog} {level} +{e:.1f} B")
    worse, total = count(open(sys.argv[1]))
    print(f"{worse} worse of {total} comparable cells")
