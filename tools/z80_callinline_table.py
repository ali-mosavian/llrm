"""Landing table: BASE vs NEW sweep dir (under ~/scratch/z80-hl, files new.<level>.tsv), per level and row, with
inside-bar before/after per axis. usage: z80_callinline_table.py BASE NEW > table.tsv   (summary on stderr)"""
import os, sys, collections
H = os.path.expanduser("~/scratch/z80-hl")
BASE, NEW = sys.argv[1], sys.argv[2]
LV = {"1": "O1", "s": "Os", "z": "Oz", "2": "O2", "3": "O3"}

def ours(path):
    d = {}
    if os.path.exists(path):
        for l in open(path):
            f = l.rstrip("\n").split("\t")
            if len(f) >= 4 and f[1] == "ours":
                d[f[0]] = (int(f[2]), int(f[3]))
    return d

bars = {}
for l in open(os.path.expanduser("~/scratch/handoff/matrix/z80-owner-table-2026-11-09.tsv")):
    f = l.rstrip("\n").split("\t")
    if l.startswith("#") or f[0] == "prog" or len(f) < 7 or not f[4]:
        continue
    bars[(f[0], f[1])] = (int(f[4]), int(f[6]))

def inside(level, ours_, bar):
    if bar is None:
        return ("?", "?")
    if level in ("O2", "O3"):  # verdict at 1 B = 3 T: one combined axis
        ok = "in" if (ours_[1] - bar[1]) + 3 * (ours_[0] - bar[0]) <= 0 else "out"
        return (ok, ok)
    return ("in" if ours_[0] <= bar[0] else "out", "in" if ours_[1] <= bar[1] else "out")

print("prog\tlevel\tbase_B\tbase_T\tnew_B\tnew_T\tdB\tdT\tbar_B\tbar_T\tB_in_before\tB_in_after\tT_in_before\tT_in_after\tleaves_bar")
left = 0; summ = collections.Counter(); rows = collections.Counter()
for lv, name in LV.items():
    base = ours(f"{H}/{BASE}/new.{lv}.tsv")
    new = ours(f"{H}/{NEW}/new.{lv}.tsv")
    if not new:
        continue
    for p in sorted(set(base) & set(new)):
        b, n = base[p], new[p]
        rows[name] += 1
        if b == n:
            summ[(name, "same")] += 1
            continue
        bar = bars.get((p, name))
        bi, ni = inside(name, b, bar), inside(name, n, bar)
        leaves = any(x == "in" and y == "out" for x, y in zip(bi, ni))
        left += leaves
        summ[(name, "moved")] += 1
        print(p, name, *b, *n, n[0] - b[0], n[1] - b[1], *(bar or ("", "")), bi[0], ni[0], bi[1], ni[1], "LEAVES" if leaves else "", sep="\t")
for name in LV.values():
    print(f"{name}: rows {rows[name]} same {summ[(name,'same')]} moved {summ[(name,'moved')]}", file=sys.stderr)
print(f"rows leaving the bar: {left}", file=sys.stderr)
