#!/usr/bin/env python3
"""gcc columns (best of 6502-gcc.tsv and 6502-gcc.noivopts.tsv) vs the other columns of 6502.tsv -> markdown. usage: vs_6502.py MATRIX_DIR OUT.md DATE [65c02]"""
import collections, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
d, out, date = sys.argv[1:4]
cmos = len(sys.argv) > 4 and sys.argv[4] == "65c02"
GCC = "gcc-65c02" if cmos else "gcc"
SFX = "-65c02" if cmos else ""
NAMES = {"ours": "ours" + SFX, "llvm-mos": "llvm-mos" + SFX, "sdcc": "sdcc" + SFX, "cc65": "cc65"}
def rows(p):
    for l in open(p):
        if l.startswith('#') or l.startswith('target'): continue
        f = l.rstrip('\n').split('\t'); yield f + [''] * (8 - len(f))
g = collections.defaultdict(list)
flagged = set()
# Both gcc columns (committed cc1, and gcc-noivopts-Os) go in: the bar is the lower of the two per metric.
for gf in ('6502-gcc.tsv', '6502-gcc.noivopts.tsv'):
    if not os.path.exists(f'{d}/{gf}'):
        continue
    for t, p, l, c, b, k, i, n in rows(f'{d}/{gf}'):
        if c.split('-noivopts')[0] != GCC or not b:
            continue
        if n.startswith(("hand helper", "table")):
            flagged.add(p)   # not a compiler win: left out of the comparison
        else:
            g[p].append((int(b), int(k), int(i) if i else None))
o = collections.defaultdict(dict)
for t, p, l, c, b, k, i, n in rows(f'{d}/6502.tsv'):
    if c in NAMES.values() and b and k:
        c = {v: k_ for k_, v in NAMES.items()}[c]
        x = o[p].setdefault(c, [10**9, 10**9, 10**9]); x[0] = min(x[0], int(b)); x[1] = min(x[1], int(k))
        if i: x[2] = min(x[2], int(i))
OURS = open(f"{d}/6502.tsv").readline().lstrip("# ").split(";")[0].strip()
L = [f"# gcc 6502 vs the other {'65C02' if cmos else 'NMOS'} columns, {date}", "",
     f"gcc = best of -O1/-O2/-Os and of both gcc columns (cc1 md5 4c91eb80 gcc, 75c808c2 gcc-noivopts-Os) per metric (bytes and clocks separately), bytes include the helpers it calls; others = min over the levels in matrix/6502.tsv ({OURS}). Same corpus and method. Instruction counts: the sim65-insns column of each (llvm-mos has none).", "",
     "| prog | gcc B | gcc clk | gcc insn | ours B/clk/insn | llvm-mos B/clk | sdcc B/clk/insn | cc65 B/clk/insn | gcc beats the best other |", "|---|---|---|---|---|---|---|---|---|"]
wb = wc = n = 0
for p in g:
    gb = min(x[0] for x in g[p]); gc = min(x[1] for x in g[p]); cols = o.get(p, {})
    def cell(c):
        if c not in cols: return "-"
        v = cols[c]
        return "%d/%d" % tuple(v[:2]) + ("" if c == 'llvm-mos' or v[2] == 10**9 else "/%d" % v[2])
    gi = min((x[2] for x in g[p] if x[2] is not None), default=None)
    ob = min((v[0] for v in cols.values()), default=None); oc = min((v[1] for v in cols.values()), default=None)
    w = "-"
    if cols:
        n += 1; a = gb < ob; b = gc < oc; wb += a; wc += b; w = ("bytes " if a else "") + ("clk" if b else "") or "none"
    L.append(f"| {p} | {gb} | {gc} | {gi if gi is not None else '-'} | {cell('ours')} | {cell('llvm-mos')} | {cell('sdcc')} | {cell('cc65')} | {w} |")
L += ["", f"Of {n} programs, gcc strictly beats the best other column in bytes on {wb} and in clocks on {wc}."]
L += ["", "Result convention: gcc cells include the caller's move of the result into A (A:X), +2 B/3 clk/1 insn, 16-bit +4 B/6 clk/2 insn (see the tsv header). Cells noted 'hand helper, not codegen' or 'table, not codegen' are left out of the gcc side: " + (", ".join(sorted(flagged - set(g))) or "no program is left without a cell") + " have no other cell."]
open(out, 'w').write("\n".join(L) + "\n"); print(L[-1])
