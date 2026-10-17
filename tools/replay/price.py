"""Priced bound per fact family from the trace: what a recompute costs under (a) an updater sized to the
measured ones (ratio 0.33 counted, 0.75 bounded) and (b) a capped per-query ask (memory families, residual 0.17
from the cap-6 fold result), keeping the same-version and irrelevant-edit rebuilds free and the first compute paid."""
import json, sys, collections, importlib.util
spec = importlib.util.spec_from_file_location('keep', __file__.replace('price.py', 'keep.py'))
src = open(__file__.replace('price.py', 'keep.py')).read().split("T = sum(")[0]
sys.argv = [sys.argv[0], sys.argv[1], sys.argv[2]]
exec(src)
TOTAL = float(__import__('os').environ.get('COMPILE_M', '2574.4')) * 1e6
PERQ = MEM | {'bounded'} - {'bounded'}
rows = []
for an, c in by.items():
    rel, irr, same, first = (c[k] for k in ('relevant edit', 'irrelevant edits only (keepable)', 'same version rebuilt', 'first (new body or lineage)'))
    paid = rel + irr + same + first
    upd_lo = first + 0.33 * rel; upd_hi = first + 0.75 * rel
    perq = first + 0.17 * rel if an in PERQ else None
    rows.append((paid, an, rel, same + irr, first, upd_lo, upd_hi, perq))
rows.sort(reverse=True)
f = lambda x: f"{x/1e6:7.1f}"
print(f"{'family':18} {'today':>7} {'after-edit':>10} {'free keeps':>10} {'first':>7} | {'updater lo':>10} {'updater hi':>10} {'per-query':>9}   (M, exclusive)")
tot = [0]*6
for paid, an, rel, free, first, lo, hi, pq in rows:
    best_lo = min(lo, pq) if pq is not None else lo
    print(f"{an:18} {f(paid)} {f(rel):>10} {f(free):>10} {f(first)} | {f(lo):>10} {f(hi):>10} {f(pq) if pq is not None else '       -':>9}")
    for i, v in enumerate((paid, lo, hi, best_lo)): tot[i] += v
print(f"{'TOTAL':18} {f(tot[0])}   updater lo {f(tot[1])}  hi {f(tot[2])}  best(lo,per-query) {f(tot[3])}")
for name, v in (('today', tot[0]), ('updater lo', tot[1]), ('updater hi', tot[2]), ('best', tot[3])):
    print(f"  {name:11} {v/1e6:7.1f} M = {100*v/TOTAL:4.1f}% of the compile; saving {100*(tot[0]-v)/TOTAL:4.1f}%")
