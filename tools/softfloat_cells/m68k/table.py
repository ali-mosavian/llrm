"""usage: table.py RESULT.md -> per op, cpu-matched cells (68000 exe on the 68000 column, 68020 exe on the 68020 column), ours vs libgcc, with the pricing status of each cell.
Status from `cycles --exe`: '*' = a data-dependent term (mulu/divu/shift) priced at its bound, so the cell is an upper bound; '?N' = N executed instructions the tables do not cover,
counted as 0, so the cell is a lower bound; the high figure adds N * MAXUNP clocks (MAXUNP=12: more than any of the unpriced forms, bfextu/bftst/extb.l/pea, costs on a 68020)."""
import re, sys
MAXUNP = 12; CALLS = 3000
OPS = {"0": "add", "1": "sub", "2": "mul", "3": "div", "4": "lt", "5": "floatsidf", "6": "floatdidf", "7": "fixdfsi", "8": "extendsfdf2", "9": "truncdfsf2", "99": "null"}
cell = {}
for l in open(sys.argv[1]):
    m = re.match(r"\| (ours|libgcc)_(68000|68020)_op(\d+) \| (\d+) \| (\d+)(\*?)(?: \?(\d+))? \| [\d.]+ \| (\d+)(\*?)(?: \?(\d+))? \| ([\d.]+) \|", l)
    if not m: continue
    col, cpu, op, insns = m[1], m[2], m[3], int(m[4])
    if cpu == "68000": clk, star, unp = int(m[5]), m[6], int(m[7] or 0)
    else: clk, star, unp = int(m[8]), m[9], int(m[10] or 0)
    cell[(col, cpu, op)] = (clk / CALLS, star, unp / CALLS, (clk + unp * MAXUNP) / CALLS)
print("| op | cpu | ours clk | status | libgcc clk (low-high) | status | ours/libgcc (vs low, vs high) |\n|---|---|---|---|---|---|---|")
for op, name in OPS.items():
    if op == "99": continue
    for cpu in ("68000", "68020"):
        o, g = cell.get(("ours", cpu, op)), cell.get(("libgcc", cpu, op))
        if not o: continue
        st = lambda c: ("upper-bound(*)" if c[1] else "exact") + (f", {c[2]:.2f} unpriced insn/call" if c[2] else "")
        if g:
            hi = f"{g[0]:.0f}-{g[3]:.0f}" if g[2] else f"{g[0]:.0f}"
            r = f"{o[0] / g[0]:.2f}x, {o[0] / g[3]:.2f}x" if g[2] else f"{o[0] / g[0]:.2f}x"
            print(f"| {name} | {cpu} | {o[0]:.0f} | {st(o)} | {hi} | {st(g)} | {r} |")
        else:
            print(f"| {name} | {cpu} | {o[0]:.0f} | {st(o)} | - (no member in lb1sf68) | - | - |")
