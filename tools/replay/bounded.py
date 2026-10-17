"""Recomputed asks of an analysis after a relevant edit: split by whether the edits since its last computation changed the CFG."""
import json, sys, collections
AN = sys.argv[2].split(','); ONLY = sys.argv[3] if len(sys.argv) > 3 else ''
last = {}; cs = collections.defaultdict(lambda: collections.defaultdict(int))
n = collections.Counter(); m = collections.Counter()
for line in open(sys.argv[1]):
    d = json.loads(line)
    if ONLY and ONLY not in d['file']: continue
    if d['ev'] == 'edit':
        for k in [k for k in last if k[:2] == (d['file'], d['fn'])]: last[k].append(d)
    elif d['ev'] == 'ask':
        c = cs[d['file']]; dep = d['depth']
        excl = d['instr'] - c[dep + 1]; c[dep + 1] = 0; c[dep] += d['instr']
        k = (d['file'], d['fn'], d['analysis'])
        if d['analysis'] in AN and d['how'] in ('recomputed', 'updated') and k in last and last[k]:
            es = last[k]
            cfg = any(e.get('cfg') or e.get('term') or e['kind'] in ('BlockCreated', 'BlockErased', 'Parameters') for e in es)
            key = (d['analysis'], d['how'], 'cfg' if cfg else 'no-cfg')
            n[key] += 1; m[key] += excl
        if d['how'] != 'hit' or k in last: last[k] = []
for k in sorted(n): print(f"{k[0]:10} {k[1]:11} {k[2]:7} {n[k]:6} asks {m[k]/1e6:8.1f} M  ({m[k]/n[k]:9.0f} per ask)")
