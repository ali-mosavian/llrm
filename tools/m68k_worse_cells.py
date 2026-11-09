#!/usr/bin/env python3
"""Count the m68k matrix cells where ours is worse than gcc or clang, strictly (no tolerance).

O1: strict, bytes and clocks (worse on either axis). Os: bytes. O2 and O3: bytes over, minus the per-CPU allowance (B per clock saved, read from
the tsv header) times the clocks saved. A cell is judged against every external number present (gcc, clang or both): a
cell with no clang number (it calls a runtime) is still judged against gcc.
Only a cell with no `ours` or no external number is skipped, and the skipped cells are
listed. `ours` is the shipped llrm-cc path; `ours-ir` (the backend on the bench IR, not
what ships) is never judged.
A cell with only one of the two external columns is named, and refused unless --allow-one-sided: judged
against the one that exists it can understate the gap to the one that is missing (z80's table did, by 1.7 kB).
Usage: m68k_worse_cells.py m68k.tsv [-v] [--allow-one-sided]
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


def excess(level: str, rate: float, ours: tuple[int, int | None], other: tuple[int, int | None]) -> float:
    """Bytes over; at O2 less the allowance for the clocks saved. A bytes-only cell (a program that calls a
    runtime routine, nothing runs) has no clocks on either side and is judged on bytes alone."""
    extra = ours[0] - other[0]
    if level == "Os" or ours[1] is None or other[1] is None:
        return extra
    if level == "O1":
        return max(extra, ours[1] - other[1])
    return extra - rate * (other[1] - ours[1])


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
            clocks = int(row[5]) if row[5] != "" else None
            seen.setdefault(key, {})[row[3]] = (int(row[4]), clocks)
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


# Programs whose branch goes either way, and the inputs the matrix must have averaged: one input makes
# mirrored branch layouts look different by the data alone (max(7, 42) judged a move that the other
# order skips).
INPUTS = {"max": 2, "absdiff": 2, "clamp": 3}


def single_input(lines) -> list[tuple[str, str, str, str]]:
    """The rows of an order-sensitive program that ran fewer inputs than INPUTS names."""
    found = []
    for line in lines:
        row = line.rstrip("\n").split("\t")
        if line.startswith("#") or len(row) < 8 or row[1] not in INPUTS or row[3] not in ("ours", "gcc", "clang"):
            continue
        if row[5] != "" and f"inputs {INPUTS[row[1]]}" not in row[7]:
            found.append((row[0], row[1], row[2], row[3]))
    return found


def ours_failed(lines) -> set[tuple[str, str, str]]:
    """The cells where `ours` has no number and says why (`n/a: compile-fail: ...`): a failure the table names, not a hole. The corpus
    matrix (tools/m68k_corpus_matrix.py) writes these."""
    return {(r[0], r[1], r[2]) for r in (l.rstrip("\n").split("\t") for l in lines if not l.startswith("#"))
            if len(r) >= 8 and r[3] == "ours" and r[4] == "" and r[7].startswith("n/a:")}


def missing(lines) -> list[tuple[str, str, str]]:
    """The cells with no `ours` number and no stated reason: a column that is absent hides its cells, so a matrix with any fails."""
    lines = list(lines)
    _, seen, keys = _seen(lines)
    named = ours_failed(lines)
    return [key for key in keys if "ours" not in seen.get(key, {}) and key not in named]


LEVELS = ("O1", "O2", "O3", "Os")


def missing_levels(lines) -> list[tuple[str, str, str]]:
    """The (target, prog, level) cells absent from a table that measured some level of that program: an unmeasured
    level is unknown, not clean (the matrix measured O2 and Os only, and O1, a strict level, went unjudged)."""
    _, seen, keys = _seen(lines)
    have = {(target, prog) for target, prog, _ in keys}
    return [(target, prog, level) for target, prog in sorted(have) for level in LEVELS if (target, prog, level) not in seen]


def one_sided(lines) -> list[tuple[str, str, str, str]]:
    """The cells with `ours` and exactly one of gcc and clang, with the column that is missing."""
    _, seen, keys = _seen(lines)
    found = []
    for key in keys:
        by = seen.get(key, {})
        if "ours" in by and ("gcc" in by) != ("clang" in by):
            found.append((*key, "clang" if "gcc" in by else "gcc"))
    return found


def count(lines) -> tuple[int, int]:
    """(worse, judged). A cell where ours failed to build and an external did is judged and worse: not building is the worst result."""
    lines = list(lines)
    results = list(cells(lines))
    _, seen, _ = _seen(lines)
    broken = [key for key in ours_failed(lines) if {"gcc", "clang"} & set(seen.get(key, {}))]
    return sum(1 for *_, e in results if e > 0) + len(broken), len(results) + len(broken)


if __name__ == "__main__":
    if "-v" in sys.argv:
        for target, prog, level, e in sorted(cells(open(sys.argv[1])), key=lambda c: -c[3]):
            if e > 0:
                print(f"{target} {prog} {level} +{e:.1f} B")
    lost = missing(open(sys.argv[1]))
    if lost:
        print("no `ours` number (the matrix is incomplete): " + ", ".join("/".join(key) for key in lost))
        sys.exit(2)
    gaps = missing_levels(open(sys.argv[1]))
    if gaps:
        print("level not measured (unknown, not clean): " + ", ".join("/".join(key) for key in gaps[:8]))
        sys.exit(2)
    once = single_input(open(sys.argv[1]))
    if once:
        print("order-sensitive program run on fewer inputs than " + str(INPUTS) + ": "
              + ", ".join("/".join(key) for key in once[:6]))
        sys.exit(2)
    half = one_sided(open(sys.argv[1]))
    if half:
        print("cells with one external column only (the best of gcc and clang is unknown): "
              + ", ".join("/".join(key[:3]) + f" (no {key[3]})" for key in half))
        if "--allow-one-sided" not in sys.argv:
            sys.exit(2)
    worse, total = count(open(sys.argv[1]))
    left = skipped(open(sys.argv[1]))
    print(f"{worse} worse of {total} judged cells; {len(left)} skipped (no ours or no external number): "
          + ", ".join("/".join(key) for key in left))
