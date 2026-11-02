#!/usr/bin/env python3
"""gcc column vs the other NMOS columns of matrix/6502.tsv -> markdown. usage: vs_6502.py MATRIX_DIR OUT.md DATE"""
import collections, sys
d, out, date = sys.argv[1:4]
def rows(p):
    for l in open(p):
        if l.startswith('#') or l.startswith('target'): continue
        f = l.rstrip('\n').split('\t'); yield f + [''] * (8 - len(f))
g = collections.defaultdict(list)
for t, p, l, c, b, k, i, n in rows(f'{d}/6502-gcc.tsv'): g[p].append((int(b), int(k)))
o = collections.defaultdict(dict)
for t, p, l, c, b, k, i, n in rows(f'{d}/6502.tsv'):
    if c in ('cc65', 'sdcc', 'llvm-mos', 'ours') and b and k:
        x = o[p].setdefault(c, [10**9, 10**9]); x[0] = min(x[0], int(b)); x[1] = min(x[1], int(k))
L = [f"# gcc 6502 vs the other NMOS columns, {date}", "",
     "gcc = best of -O1/-O2/-Os per metric (bytes and clocks separately), bytes include the helpers it calls; others = min over the levels in matrix/6502.tsv. Same corpus and method. Instrs not compared.", "",
     "| prog | gcc B | gcc clk | ours B/clk | llvm-mos B/clk | sdcc B/clk | cc65 B/clk | gcc beats the best other |", "|---|---|---|---|---|---|---|---|"]
wb = wc = n = 0
for p in g:
    gb = min(x[0] for x in g[p]); gc = min(x[1] for x in g[p]); cols = o.get(p, {})
    cell = lambda c: ("%d/%d" % tuple(cols[c])) if c in cols else "-"
    ob = min((v[0] for v in cols.values()), default=None); oc = min((v[1] for v in cols.values()), default=None)
    w = "-"
    if cols:
        n += 1; a = gb < ob; b = gc < oc; wb += a; wc += b; w = ("bytes " if a else "") + ("clk" if b else "") or "none"
    L.append(f"| {p} | {gb} | {gc} | {cell('ours')} | {cell('llvm-mos')} | {cell('sdcc')} | {cell('cc65')} | {w} |")
L += ["", f"Of {n} programs, gcc strictly beats the best other column in bytes on {wb} and in clocks on {wc}."]
open(out, 'w').write("\n".join(L) + "\n"); print(L[-1])
