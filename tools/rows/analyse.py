"""Rows series: llrm at a base commit against llrm at another, per level, against min(gcc, clang). ROWS_WORK holds base/ and main/ (run.sh), df-*.json, qcport.json."""
import json,math,os,sys
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
import rowcheck
W=os.environ.get('ROWS_WORK',os.path.expanduser('~/scratch/rows'))+'/'
def load(n):
    P={}
    for l in open(W+n+'/results.jsonl'):
        if l.startswith('{'):
            r=json.loads(l); P.setdefault(r['prog'],{})[r['variant']]=r
    return P
base,main=load('base'),load('main')
flat=lambda P:{f"{p}/{v}":r for p,vs in P.items() for v,r in vs.items()}
line,bad=rowcheck.census({'base':flat(base),'main':flat(main)})
# Printed first; a failed or missing row stops the table unless --allow-failed.
head=rowcheck.require(line,bad,'--allow-failed' in sys.argv)
LV=('O1','O2','O3','Os'); L=lambda l:'llrm' if l=='O2' else 'llrm'+l
def ref(P,p,l,k): return max(1,min(P[p]['gcc'+l][k],P[p]['clang'+l][k]))
def gm(xs): return math.exp(sum(math.log(x) for x in xs)/len(xs))
out=list(head)
for k,name in (('clocks','clocks'),('code','code bytes')):
    out.append(f"\n{name}: llrm / min(gcc, clang) per row, 71 programs\n")
    out.append("| level | geomean base | geomean main | worst base | worst main | rows >1.00 base | main |")
    out.append("|---|--:|--:|---|---|--:|--:|")
    for l in LV:
        rb={p:base[p][L(l)][k]/ref(main,p,l,k) for p in main}; rm={p:main[p][L(l)][k]/ref(main,p,l,k) for p in main}
        wb=max(rb,key=rb.get); wm=max(rm,key=rm.get)
        out.append(f"| -{l} | {gm(rb.values()):.4f} | {gm(rm.values()):.4f} | {rb[wb]:.2f} ({wb}) | {rm[wm]:.2f} ({wm}) | {sum(v>1 for v in rb.values())} | {sum(v>1 for v in rm.values())} |")
out.append("\nrows whose llrm number changed (base -> main), by counter:\n")
for l in LV:
    for k in ('clocks','ins','mem','code'):
        ch=[(p,base[p][L(l)][k],main[p][L(l)][k]) for p in main if base[p][L(l)][k]!=main[p][L(l)][k]]
        if ch: out.append(f"-{l} {k}: {len(ch)} rows changed: "+", ".join(f"{p} {a}->{b} ({(b/max(1,a)-1)*100:+.2f}%)" for p,a,b in sorted(ch,key=lambda t:t[2]/max(1,t[1]))))
out.append("\nrows newly below / above min(gcc, clang) clocks (ratio crosses 1.00):\n")
for l in LV:
    nb=[];na=[]
    for p in main:
        r0=base[p][L(l)]['clocks']/ref(main,p,l,'clocks'); r1=main[p][L(l)]['clocks']/ref(main,p,l,'clocks')
        if r0>=1>r1: nb.append(f"{p} {r0:.3f}->{r1:.3f}")
        if r0<1<=r1: na.append(f"{p} {r0:.3f}->{r1:.3f}")
    out.append(f"-{l}: newly below (better): {nb or 'none'}; newly above (worse): {na or 'none'}")
# d_faces
def dl(n): return [json.loads(l) for l in open(W+f'df-{n}.json') if l.startswith('{')]
db,dm=dl('base'),dl('main'); key=lambda r:(r['variant'])
out.append("\nd_faces executed, llrm variants (base -> main): "+("IDENTICAL on every counter" if {key(r):(r['ins'],r['mem'],r['misaligned'],r['clocks'],r['code']) for r in db}=={key(r):(r['ins'],r['mem'],r['misaligned'],r['clocks'],r['code']) for r in dm} else "DIFFERENT"))
D={r['variant']:r for r in dm}
for l in LV:
    v=L(l); best=min(D['gcc'+l]['clocks'],D['clang'+l]['clocks']); b={r['variant']:r for r in db}[v]
    out.append(f"  -{l}: clocks base {b['clocks']} main {D[v]['clocks']}; min(gcc,clang) {best}; ratio {D[v]['clocks']/best:.3f}; code {D[v]['code']} (gcc {D['gcc'+l]['code']}, clang {D['clang'+l]['code']})")
# qcport
q=json.load(open(W+'qcport.json')); Q={}
for n,m,l,t in q: Q.setdefault(l,{}).setdefault(m,{})[n]=t
out.append("\nQCport modules, emitted bytes (LEDATA), 65 modules: base -> main\n")
for l in ("-O1","-O2","-O3","-Os"):
    b=sum(v['base'] for v in Q[l].values()); m=sum(v['main'] for v in Q[l].values()); ch=[(k,v['base'],v['main']) for k,v in Q[l].items() if v['base']!=v['main']]
    out.append(f"{l}: total {b} -> {m} ({(m/b-1)*100:+.3f}%), {len(ch)} modules changed"+("".join(f"; {k} {x}->{y}" for k,x,y in ch[:6])))
if __name__ == '__main__':
    print("\n".join(out))
