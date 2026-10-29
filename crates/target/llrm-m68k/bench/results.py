#!/usr/bin/env python3
"""Rewrites the tables of RESULTS.md from `cargo run -p llrm-m68k --example bench` output on stdin."""
import re
import subprocess
import sys

lines = [l for l in sys.stdin.read().splitlines() if l.startswith("|")]
rows = [[c.strip() for c in l.strip("|").split("|")] for l in lines[2:]]
# kernel, raw B, ours B, ours -Os B, llc -O2 B, llc -Os B, gcc -Os B, gcc -O2 B,
# raw insns, ours insns, ours -Os insns, llc -O2, llc -Os, gcc -Os, gcc -O2, result
out = [
    "## Summary: every row against the best reference at its level",
    "",
    "Speed level (`ours`, `raw`) against `llc -O2` and `gcc -O2`; size level (`ours -Os`: `optsize`/`minsize` or"
    " `Options::size`) against `llc -Os` and `gcc -Os`.",
    "",
    "| kernel | raw B | ours B | best -O2 B | ours -Os B | best -Os B | raw insns | ours insns | best -O2 insns"
    " | ours -Os insns | best -Os insns | status |",
    "|---|---|---|---|---|---|---|---|---|---|---|---|",
]
above = []
for r in rows:
    k, raw, ours, small = r[0], int(r[1]), int(r[2]), int(r[3])
    llc2, llcs, gccs, gcc2 = (int(x) for x in r[4:8])
    raw_i, ours_i, small_i = int(r[8]), int(r[9]), int(r[10])
    il2, ils, igs, ig2 = (int(x) for x in r[11:15])
    b2, bs, i2, isz = min(llc2, gcc2), min(llcs, gccs), min(il2, ig2), min(ils, igs)
    status = []
    for label, have, best, unit in (
        ("-O2", ours, b2, "B"), ("-O2", ours_i, i2, "insns"), ("-Os", small, bs, "B"), ("-Os", small_i, isz, "insns")
    ):
        if have > best:
            status.append(f"{label} +{have - best} {unit}")
    if status:
        above.append(k)
    out.append(
        f"| {k} | {raw} | {ours} | {b2} | {small} | {bs} | {raw_i} | {ours_i} | {i2} | {small_i} | {isz} |"
        f" {', '.join(status) or 'at or below'} |"
    )
out += ["", f"Rows still above a reference: {', '.join(above) or 'none'}.", ""]

text = open("bench/RESULTS.md").read()
head = text[: text.index("## Summary")]
tail = text[text.index("\n## Above") :]
note = "## Full table\n\n"
stamp = subprocess.check_output(["date", "+%H:%M"]).decode().strip()
head = re.sub(r"# Results, 2026-10-29 \d\d:\d\d", f"# Results, 2026-10-29 {stamp}", head, count=1)
open("bench/RESULTS.md", "w").write(head + "\n".join(out) + "\n" + note + "\n".join(lines) + "\n" + tail)
