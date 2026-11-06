#!/usr/bin/env python3
"""Count the m68k matrix cells where ours is worse than gcc or clang, strictly (no tolerance).

Os: bytes. O2 and above: bytes over, minus the per-CPU allowance (B per clock saved, read from
the tsv header) times the clocks saved. A cell is judged against every external number present (gcc, clang or both): a
cell with no clang number (it calls a runtime) is still judged against gcc.
Only a cell with no `ours` or no external number is skipped, and the skipped cells are
listed. `ours` is the shipped llrm-cc path; `ours-ir` (the backend on the bench IR, not
what ships) is never judged.
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


def _seen(lines):
    header, rows = "", []
    for line in lines:
        if line.startswith("#"):
            header += line
        else:
            rows.append(line.rstrip("\n").split("\t"))
    seen: dict[tuple[str, str, str], dict[str, tuple[int, int]]] = {}
    keys: list[tuple[str, str, str]] = []
    for row in rows[1:]:
        try:
            key = (row[0], row[1], row[2])
        except IndexError:
            continue
        if key not in keys:
            keys.append(key)
        try:
            seen.setdefault(key, {})[row[3]] = (int(row[4]), int(row[5]))
        except (ValueError, IndexError):
            continue
    return rates(header), seen, keys


def cells(lines):
    table, seen, _ = _seen(lines)
    for (target, prog, level), by in seen.items():
        others = [by[name] for name in ("gcc", "clang") if name in by]
        if "ours" in by and others:
            rate = table[target.split("-")[1]]
            yield target, prog, level, max(excess(level, rate, by["ours"], other) for other in others)


def skipped(lines) -> list[tuple[str, str, str]]:
    """The cells with no `ours` number or no external number: not judged."""
    _, seen, keys = _seen(lines)
    return [key for key in keys if "ours" not in seen.get(key, {}) or not ({"gcc", "clang"} & set(seen.get(key, {})))]


def count(lines) -> tuple[int, int]:
    results = list(cells(lines))
    return sum(1 for *_, e in results if e > 0), len(results)


if __name__ == "__main__":
    if "-v" in sys.argv:
        for target, prog, level, e in sorted(cells(open(sys.argv[1])), key=lambda c: -c[3]):
            if e > 0:
                print(f"{target} {prog} {level} +{e:.1f} B")
    worse, total = count(open(sys.argv[1]))
    left = skipped(open(sys.argv[1]))
    print(f"{worse} worse of {total} judged cells; {len(left)} skipped (no ours or no external number): "
          + ", ".join("/".join(key) for key in left))
