#!/usr/bin/env python3
"""The m68k card: how many cells are at or under the best compiler, by the rulings, from `m68k_clk.py` output.

m68k_card.py CLK.tsv [CLK.tsv ...]      (cpu, program, level, ours_B, ours_clk, gcc_B, gcc_clk per row)

A cell counts at or under the best when each judged axis is at most 1% over it (user ruling 2026-11-14, counted rule of oversight):
-Os judges bytes AND clocks, -O1/-O2/-O3 judge clocks (bytes are reported, not counted). The reference is gcc: the corpus matrix has no
clang clocks. A judged axis with no number on either side is unmeasured: named, never counted as at or under.
`tools/m68k_worse_cells.py` reads the bytes-only corpus matrix and judges -O1 strictly and -O2/-O3 by bytes less a clock allowance, so it
counts huffman_decode -O2 (1440 B, 136430 clk against gcc 526 B, 145262 clk) as worse.
"""
import sys

OVER = 101  # at most 1% over: ours * 100 <= best * 101


def number(text):
    return int(text) if text.strip().isdigit() else None


def over(ours, best):
    """Whether ours is more than 1% above best on one axis; None when either side is unmeasured."""
    if ours is None or best is None:
        return None
    return ours * 100 > best * OVER


def judge(level, ours_bytes, ours_clocks, gcc_bytes, gcc_clocks):
    """One of at_or_below, bytes_worse, clocks_worse, both_worse, unmeasured."""
    clocks = over(ours_clocks, gcc_clocks)
    if level == "Os":
        bytes_ = over(ours_bytes, gcc_bytes)
        if bytes_ and clocks:
            return "both_worse"
        if bytes_:
            return "bytes_worse"
        if clocks:
            return "clocks_worse"
        return "unmeasured" if bytes_ is None or clocks is None else "at_or_below"
    if clocks is None:
        return "unmeasured"
    return "clocks_worse" if clocks else "at_or_below"


def excess(level, ours_bytes, ours_clocks, gcc_bytes, gcc_clocks):
    """The overshoot in percent, on the axis the level judges (the worse one at -Os), for ranking."""
    pairs = [(ours_clocks, gcc_clocks)] + ([(ours_bytes, gcc_bytes)] if level == "Os" else [])
    return max((100.0 * (a / b - 1) for a, b in pairs if a is not None and b), default=0.0)


def read(paths):
    rows = []
    for path in paths:
        for line in open(path):
            cells = line.rstrip("\n").split("\t")
            if cells[0] == "cpu" or len(cells) < 7:
                continue
            cpu, program, level = cells[:3]
            rows.append((cpu, program, level, *(number(cell) for cell in cells[3:7])))
    return rows


def card(rows):
    """{verdict: [(excess, cpu, program, level, ours_bytes, ours_clocks, gcc_bytes, gcc_clocks)]} over every row."""
    found = {"at_or_below": [], "bytes_worse": [], "clocks_worse": [], "both_worse": [], "unmeasured": []}
    for cpu, program, level, *figures in rows:
        found[judge(level, *figures)].append((excess(level, *figures), cpu, program, level, *figures))
    return found


def main(paths):
    found = card(read(paths))
    total = sum(len(cells) for cells in found.values())
    print(f"cells {total}: at_or_below {len(found['at_or_below'])}, bytes_worse {len(found['bytes_worse'])}, "
          f"clocks_worse {len(found['clocks_worse'])}, both_worse {len(found['both_worse'])}, unmeasured {len(found['unmeasured'])}")
    for name in ("clocks_worse", "bytes_worse", "both_worse"):
        for e, cpu, program, level, ob, oc, gb, gc in sorted(found[name], reverse=True)[:5]:
            print(f"{name} {cpu} {program} {level} +{e:.1f}%  ours {ob} B {oc} clk, gcc {gb} B {gc} clk")
    print("unmeasured: " + ", ".join(f"{c}/{p}/{l}" for _, c, p, l, *_ in found["unmeasured"][:12]))


if __name__ == "__main__":
    main(sys.argv[1:])
