"""Event replay: of the recomputed asks, how many (and how many instructions) would a
log-derived keep have saved, under a declared-read model per analysis."""
import json, sys, collections
CFG = {'dominators','loops','shape','followers','dominated-edges','terminal-sites'}
MEM = {'accesses','annotated','through-memory','writes','plain-accesses','pointer-values',
       'points-to','exposed-frames','table-loads','assumptions','published'}
def relevant(an, e):
    if an in CFG: return e.get('cfg') or e.get('term') or e['kind'] in ('BlockCreated','BlockErased','Parameters')
    if an in MEM:
        return (e.get('cfg') or e.get('term') or e.get('params') or e.get('mem') or e.get('ptr')
                or e['kind'] in ('BlockCreated','BlockErased','Parameters','Substituted'))
    return True                      # every other analysis: any edit
childsum = collections.defaultdict(lambda: collections.defaultdict(int))
ONLY = sys.argv[2] if len(sys.argv)>2 else ''
last = {}                            # (file,fn,an) -> edits since last computation
pend = collections.defaultdict(list) # (file,fn) -> edits since start, with stamp
tot = collections.Counter(); ins = collections.Counter()
for line in open(sys.argv[1]):
    d = json.loads(line)
    if ONLY and ONLY not in d['file']: continue
    if d['ev'] == 'edit':
        for k in [k for k in last if k[:2] == (d['file'], d['fn'])]:
            last[k].append(d)
    elif d['ev'] == 'ask':
        an = d['analysis']; k = (d['file'], d['fn'], an)
        cs = childsum[d['file']]; dep = d['depth']
        excl = d['instr'] - cs[dep+1]; cs[dep+1] = 0; cs[dep] += d['instr']
        if d['how'] == 'recomputed':
            seen = k in last
            es = last.get(k, [])
            if not seen: cls = 'first (new body or lineage)'
            elif not es: cls = 'same version rebuilt'
            elif not any(relevant(an, e) for e in es): cls = 'irrelevant edits only (keepable)'
            else: cls = 'relevant edit'
            tot[cls] += 1; ins[cls] += excl
        if d['how'] != 'hit' or k in last: last[k] = []
T = sum(ins.values())
for c in tot: print(f"{c:36} {tot[c]:7} asks {ins[c]/1e6:9.1f} M inclusive ({100*ins[c]/T:4.1f}%)")
