#!/usr/bin/env python3
"""Count the m68k matrix cells where ours is worse than gcc or clang, strictly (no tolerance).

O1: strict, bytes and clocks (worse on either axis). Os: bytes. O2 and O3: bytes over, minus the per-CPU allowance (B per clock saved, read from
the tsv header) times the clocks saved. A cell is judged against every external number present (gcc, clang or both): a
cell with no clang number (it calls a runtime) is still judged against gcc.
Only a cell with no `ours` or no external number is skipped, and the skipped cells are
listed. `ours` is the shipped llrm-cc path; `ours-ir` (the backend on the bench IR, not
what ships) is never judged.
Bytes are the linked size: the object's code plus the self-contained compiler-rt members it pulls (the matrix's ninth field), as
each compiler is shipped. A column whose note says "unresolved helpers" has no linked size and counts as missing (a
reference then makes its cell one-sided; ours makes the matrix incomplete).
A cell with only one of the two external columns is named, and refused unless --allow-one-sided: judged
against the one that exists it can understate the gap to the one that is missing (z80's table did, by 1.7 kB).
O1 cells follow the VIRTUAL-OK rule of tools/virtual_ok.py (verdict()).
Usage: m68k_worse_cells.py m68k.tsv [-v] [--allow-one-sided]
-v lists each worse cell with both axes (dB, dT) and, at O2/O3, the net; the legend says how each level is judged.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from virtual_ok import virtual_ok  # noqa: E402

# The note tools/m68k_corpus_matrix.py writes when a cell's object calls a compiler-rt helper the archive lacks.
UNRESOLVED = "unresolved helpers"
RATES = re.compile(r"byte_rate: (.*?) B per clk")
TIMINGS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "target", "llrm-m68k", "src", "timings.times")


def committed_rates(path: str = TIMINGS) -> dict[str, float]:
    """Bytes per clock saved of each CPU, from the committed table (`cache_line` over `line_fill_clocks`)."""
    rows: dict[str, list[str]] = {}
    for line in open(path):
        words = line.split()
        if words and words[0] in ("march", "cache_line", "line_fill_clocks"):
            rows[words[0]] = words[1:]
    return {"68" + cpu[-3:]: int(line) / int(fill) for cpu, line, fill in zip(rows["march"], rows["cache_line"], rows["line_fill_clocks"])}


def rate_text(path: str = TIMINGS) -> str:
    """The `byte_rate:` text of a table header, read from the committed `timings.times`, never typed: `68000/010 0.5, 68020 1.33, 030/040/060
    3.2`. Every table writer prints this, and `rates` reads it back."""
    groups: list[tuple[list[str], float]] = []
    for cpu, value in committed_rates(path).items():
        value = round(value, 2)
        if groups and groups[-1][1] == value:
            groups[-1][0].append(cpu)
        else:
            groups.append(([cpu], value))
    return ", ".join("/".join([names[0], *(name[2:] for name in names[1:])]) + f" {value:g}" for names, value in groups)


def rates(header: str) -> dict[str, float]:
    found = RATES.search(header)
    if not found:
        raise SystemExit("no byte_rate in the header")
    table: dict[str, float] = {}
    for part in found.group(1).split(","):
        names, value = part.split()
        for name in names.split("/"):
            table[name if name.startswith("68") else "68" + name] = float(value)
    # a header that states a rate other than the committed one flips every O2/O3 verdict (11-09: a flat "2 B per clk" read 68000 partition O2 as +82 B, +25 B at the committed 0.5)
    committed = committed_rates()
    wrong = {cpu: (value, committed[cpu]) for cpu, value in table.items() if cpu in committed and abs(value - committed[cpu]) > 0.01 * committed[cpu]}
    if wrong:
        raise SystemExit(f"byte_rate in the header differs from timings.times (stated, committed): {wrong}")
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


def verdict(level: str, rate: float, ours, refs: dict) -> str:
    """"ok", "VIRTUAL-OK" or "WORSE" for one cell (oversight, 2026-11-09, as fputable.verdict for the m32 FPU table).

    `ours` and each of `refs` (gcc, clang) are (bytes, clocks). O1 has a per-axis bar and the rule of tools/virtual_ok.py
    (pin: clang). Other levels, and a cell without both columns or without clocks, are one metric.
    """
    if max(excess(level, rate, ours, ref) for ref in refs.values()) <= 0:
        return "ok"
    gcc, clang = refs.get("gcc"), refs.get("clang")
    if level != "O1" or gcc is None or clang is None or None in (*ours, *gcc, *clang):
        return "WORSE"
    return "VIRTUAL-OK" if virtual_ok(ours, [gcc, clang], clang) else "WORSE"


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
            if len(row) > 7 and row[7].startswith(UNRESOLVED):
                continue  # a helper the archive lacks: the linked size is unknown, not small
            clocks = int(row[5]) if row[5] != "" else None
            library = int(row[8]) if len(row) > 8 and row[8] != "" else 0
            seen.setdefault(key, {})[row[3]] = (int(row[4]) + library, clocks)
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


def verdicts(lines):
    """(target, prog, level, verdict, excess) of every judged cell."""
    table, seen, _ = _seen(lines)
    for (target, prog, level), by in seen.items():
        refs = {name: by[name] for name in ("gcc", "clang") if name in by}
        if "ours" in by and refs:
            rate = table[target.split("-")[1]]
            worst = max(excess(level, rate, by["ours"], ref) for ref in refs.values())
            yield target, prog, level, verdict(level, rate, by["ours"], refs), worst


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


def missing(lines) -> list[tuple[str, str, str]]:
    """The cells with no `ours` number: a column that is absent hides its cells, so a matrix with any fails."""
    _, seen, keys = _seen(lines)
    return [key for key in keys if "ours" not in seen.get(key, {})]


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


LEGEND = (
    "# dB, dT: ours minus the best external on that axis (bytes, clocks); negative is better. Judged per level:",
    "# O1: strict per axis (worse on dB or dT), VIRTUAL-OK rule of tools/virtual_ok.py",
    "# Os: bytes (dB) only",
    "# O2, O3: net = excess bytes - byte_rate * clocks saved, against the external it is worst on; byte_rate per CPU from timings.times",
)


def axes(ours, refs: dict) -> tuple[int, int | None]:
    """(dB, dT): ours less the best bytes among the references, ours less the best clocks (None when a side has none)."""
    clocks = [ref[1] for ref in refs.values() if ref[1] is not None]
    return ours[0] - min(ref[0] for ref in refs.values()), (None if ours[1] is None or not clocks else ours[1] - min(clocks))


def report(lines) -> list[str]:
    """One line per cell that is not ok: both axes and, at O2 and O3, the net the verdict is made on. The net is bytes
    and the O1 excess is whichever axis is worse, so a bare number would mix bytes and clocks (crc16 O1 read "+52 B" for
    10 B better and 52 clk worse)."""
    lines = list(lines)
    table, seen, _ = _seen(lines)
    found = []
    for target, prog, level, v, e in sorted(verdicts(lines), key=lambda c: -c[4]):
        if v == "ok":
            continue
        by = seen[(target, prog, level)]
        refs = {name: by[name] for name in ("gcc", "clang") if name in by}
        d_bytes, d_clocks = axes(by["ours"], refs)
        text = f"{target} {prog} {level} {v} dB={d_bytes:+d} B dT=" + ("n/a" if d_clocks is None else f"{d_clocks:+d} clk")
        if level in ("O2", "O3"):
            text += f" net={e:+.1f} B at {table[target.split('-')[1]]:g} B/clk"
        found.append(text)
    return found


def count(lines) -> tuple[int, int]:
    results = list(verdicts(lines))
    return sum(1 for _, _, _, v, _ in results if v == "WORSE"), len(results)


if __name__ == "__main__":
    if "-v" in sys.argv:
        print("\n".join(LEGEND))
        print("\n".join(report(open(sys.argv[1]))))
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
