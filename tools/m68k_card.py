#!/usr/bin/env python3
"""The m68k card: how many cells are at or under the best compiler, by the rulings, from `m68k_clk.py` output.

m68k_card.py CLK.tsv [CLK.tsv ...]      (cpu, program, level, ours_B, ours_clk, gcc_B, gcc_clk, ours_lib, gcc_lib per row)

Bytes are whole-program: the object's code plus the compiler-rt members it links (`ours_lib`, `gcc_lib`: gcc calls `__ashldi3` where ours
inlines the shift, shift64_variable -Os: 416 B code against gcc's 330 B code + 216 B of libcalls). A row without the lib columns counts code only; an empty lib cell (the link failed) leaves that side's bytes unmeasured.

A cell counts at or under the best when each judged axis is at most 1% over it (user ruling 2026-11-14, counted rule of oversight):
-Os judges bytes AND clocks, -O1/-O2/-O3 judge clocks (bytes are reported, not counted). The reference is gcc: the corpus matrix has no
clang clocks. A judged axis with no number on either side is unmeasured: named, never counted as at or under.
`tools/m68k_worse_cells.py` reads the bytes-only corpus matrix and judges -O1 strictly and -O2/-O3 by bytes less a clock allowance, so it
counts huffman_decode -O2 (1440 B, 136430 clk against gcc 526 B, 145262 clk) as worse.
"""
import sys

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from m68k_worse_cells import settle_bounds  # noqa: E402

OVER = 101  # at most 1% over: ours * 100 <= best * 101


def number(text):
    """An exact clocks or bytes cell; a bound (`<=N`, `>=N`) is kept as text for `settle_bounds`."""
    text = text.strip()
    return int(text) if text.isdigit() else text if text[:2] in ("<=", ">=") and text[2:].isdigit() else None


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


def whole(code, library):
    """Code plus library bytes; None (unmeasured) when either is: a link that failed on a helper the archive lacks leaves the whole-program size unknown, not the code alone (widening_multiply -Os read ours 1587 B + 1366 B of library against gcc's bare 339 B)."""
    return None if code is None or library is None else code + library


def read(paths):
    rows = []
    for path in paths:
        for line in open(path):
            cells = line.rstrip("\n").split("\t")
            if cells[0] == "cpu" or len(cells) < 7:
                continue
            cpu, program, level = cells[:3]
            ours_bytes, ours_clocks, gcc_bytes, gcc_clocks = (number(cell) for cell in cells[3:7])
            ours_lib, gcc_lib = (number(cells[at]) if len(cells) > at else 0 for at in (7, 8))
            sides = {"ours": (0, ours_clocks), "gcc": (0, gcc_clocks)}
            settle_bounds(sides)
            ours_clocks, gcc_clocks = sides["ours"][1], sides["gcc"][1]
            rows.append((cpu, program, level, whole(ours_bytes, ours_lib), ours_clocks, whole(gcc_bytes, gcc_lib), gcc_clocks))
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
