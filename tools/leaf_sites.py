"""Where a compile's container cost is, by site.

    valgrind --tool=callgrind --callgrind-out-file=cg.out llrm-c -m32 -O2 -o file.obj file.c
    python3 tools/leaf_sites.py cg.out [N]

Callgrind's exclusive cost of the container code (allocator, memcpy, B-trees, Vec growth, hash tables, drop glue) is pushed up the call
graph, through container frames only, to the first caller that is not container code, in proportion to the cost of each edge. Prints the
N largest sites with their cost by class. docs/architecture/compile-time-spec.md section 9 has the table for main.
"""
import re,sys,collections
path=sys.argv[1]
names={}          # id->name
selfc=collections.Counter()
edges=collections.defaultdict(collections.Counter)  # callee -> caller -> inclusive cost
cur=None; callee=None; pend=None
def nm(tok):
    m=re.match(r'\((\d+)\)(?: (.*))?',tok)
    if not m: return tok
    if m[2]: names[m[1]]=m[2]
    return names.get(m[1],m[1])
for line in open(path):
    line=line.rstrip('\n')
    if line.startswith('fn='):
        cur=nm(line[3:]); pend=None
    elif line.startswith('cfn='):
        callee=nm(line[4:])
    elif line.startswith('calls='):
        pend=True
    elif line and (line[0].isdigit() or line[0] in '+-*'):
        parts=line.split()
        if len(parts)>=2:
            cost=int(parts[1])
            if pend:
                edges[callee][cur]+=cost; pend=None
            else:
                selfc[cur]+=cost
total=sum(selfc.values())
classes=[("alloc",r"_mi_|mi_(free|malloc|heap|theap|page|realloc|zalloc|usable|block)|__rust_(alloc|dealloc|realloc)|__rdl|__rustc::__rust"),
 ("mem",r"memmove|memcpy|memset|memcmp|bcmp"),
 ("btree",r"btree"),
 ("vecgrow",r"RawVec|raw_vec|finish_grow|grow_one|reserve"),
 ("hash",r"hashbrown|indexmap|FxHash|SipHasher|BuildHasher|hash_one|siphash|RawTable"),
 ("drop",r"drop_glue|drop_in_place|drop_slow")]
def klass(n):
    for k,p in classes:
        if re.search(p,n): return k
    return None
# push container self cost up to first non-container caller
site=collections.defaultdict(collections.Counter)
memo={}
def share(fn,depth=0):
    # distribution of one unit of cost of fn over non-container callers
    if fn in memo: return memo[fn]
    callers=edges.get(fn)
    if not callers or depth>12:
        memo[fn]={"(root)":1.0}; return memo[fn]
    t=sum(callers.values()) or 1
    out=collections.Counter()
    for c,w in callers.items():
        if klass(c) or c==fn:
            sub=share(c,depth+1) if c!=fn else {"(self)":1.0}
            for k,v in sub.items(): out[k]+=v*w/t
        else:
            out[c]+=w/t
    memo[fn]=out; return out
for fn,c in selfc.items():
    k=klass(fn)
    if k:
        for s,f in share(fn).items(): site[s][k]+=c*f
rows=sorted(site.items(),key=lambda kv:-sum(kv[1].values()))
print("total",total)
for s,d in rows[:int(sys.argv[2]) if len(sys.argv)>2 else 40]:
    t=sum(d.values())
    print(f"{t/1e6:7.1f}M {100*t/total:4.1f}%  "+" ".join(f"{k}={v/1e6:.1f}" for k,v in d.most_common())+"  "+re.sub(r'\s+',' ',s)[:130])
