#!/usr/bin/env python3
"""cells_worse.py A.json B.json -> the cells where B is worse than A on code bytes, clocks or instructions (tools/bench/bench.py --json files).
Exit 1 when there is one; a cell present in A and failing in B counts."""
import json, sys

METRICS = ('code_bytes', 'clocks', 'instructions')

def worse(a, b):
    out = []
    for cell, x in sorted(a.items()):
        y = b.get(cell)
        if 'error' in x or 'skip' in x:
            continue
        if y is None or 'error' in y or 'skip' in y:
            out.append((cell, 'missing or failing in B', '', ''))
            continue
        for m in METRICS:
            if m in x and m in y and y[m] > x[m]:
                out.append((cell, m, x[m], y[m]))
    return out

if __name__ == '__main__':
    rows = worse(json.load(open(sys.argv[1])), json.load(open(sys.argv[2])))
    for r in rows:
        print('WORSE %s %s %s -> %s' % r)
    print('worse cells: %d' % len(rows))
    sys.exit(1 if rows else 0)
