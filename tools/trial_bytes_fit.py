"""Fit the per-class byte prices of the -Os trial's size model to assembled objects.

    python3 tools/trial_bytes_fit.py ALL.TSV [MAX_BYTES]

ALL.TSV: `file TAB function TAB object bytes TAB class counts` (`k=n k=n ...`), one line per function. Made by building
each source with a temporary `LLRM_FB_DUMP` patch in `walk.rs` (before `managers(program, analyses)`: for every function
print `FH name key=count ...` for the classes `function_bytes` prices: alu, br, call, callargs, div, ext, fcmp, fpcast, gep,
icmp, load, mul, other, phi, ret, select, shift, store, switch) and joining the last line per function with `nm -S`
sizes of the object. Non-negative least squares by dropping the most negative class until none is. Used for
`crates/target/llrm-x86-m64/src/opcosts.txt` (X64-BYTES): functions of at most 300 B, 1228 of them.
"""
import sys, statistics as st
rows=[]
for l in open(sys.argv[1]):
    f,n,a,h=l.rstrip('\n').split('\t',3)
    c={}
    for kv in h.split():
        k,v=kv.split('='); c[k]=int(v)
    rows.append((n,int(a),c))
keys=sorted({k for _,_,c in rows for k in c if k not in ('free','castfree','gepfold')})
rows=[r for r in rows if r[1]<=int(sys.argv[2] if len(sys.argv)>2 else 300)]
X=[[c.get(k,0) for k in keys]+[1] for n,a,c in rows]; y=[a for n,a,c in rows]
m=len(keys)+1
def solve(active):
    idx=[i for i in range(m) if active[i]]
    A=[[sum(X[r][i]*X[r][j] for r in range(len(X))) for j in idx] for i in idx]
    b=[sum(X[r][i]*y[r] for r in range(len(X))) for i in idx]
    k=len(idx)
    for i in range(k):
        p=max(range(i,k),key=lambda r:abs(A[r][i])); A[i],A[p]=A[p],A[i]; b[i],b[p]=b[p],b[i]
        d=A[i][i] or 1e-9
        for r in range(i+1,k):
            f=A[r][i]/d
            for c in range(i,k): A[r][c]-=f*A[i][c]
            b[r]-=f*b[i]
    x=[0]*k
    for i in range(k-1,-1,-1):
        x[i]=(b[i]-sum(A[i][c]*x[c] for c in range(i+1,k)))/(A[i][i] or 1e-9)
    out=[0]*m
    for i,j in enumerate(idx): out[j]=x[i]
    return out
active=[True]*m
for _ in range(m):
    co=solve(active)
    neg=[i for i in range(m) if active[i] and co[i]<0 and i!=m-1]
    if not neg: break
    active[min(neg,key=lambda i:co[i])]=False
co=solve(active)
pred=[sum(co[i]*X[r][i] for i in range(m)) for r in range(len(X))]
print('n',len(rows),'median abs err',st.median(abs(p-a) for p,a in zip(pred,y)))
for k,v in zip(keys+['const'],co): print(f'{k:8s} {v:6.2f}')
