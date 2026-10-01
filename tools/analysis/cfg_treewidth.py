"""Upper bound on each function's CFG treewidth (min-degree elimination).

Input: `[cfg]` lines from `LLRM_DEBUG=cfg llrm-c|llrm-qb ... 2>&1`, on stdin.
Prints `name blocks treewidth`, widest first.
"""
import sys
def tw(adj):
    adj={k:set(v) for k,v in adj.items()};w=0
    while adj:
        v=min(adj,key=lambda x:len(adj[x]));n=adj.pop(v);w=max(w,len(n))
        for a in n:
            adj[a].discard(v);adj[a]|=n-{a}
    return w
rows=[]
for l in sys.stdin:
    name,*t=l.split()[1:]
    adj={}
    for x in t:
        if '>' in x:
            a,b=x.split('>')
            if a!=b: adj.setdefault(a,set()).add(b);adj.setdefault(b,set()).add(a)
        else: adj.setdefault(x,set())
    rows.append((name,len(adj),tw(adj)))
for r in sorted(rows,key=lambda r:-r[2]):print(*r)
