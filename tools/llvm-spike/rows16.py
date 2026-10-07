import json,sys
G='/home/alim/scratch/llvm-spike/gate/'
tag,lvl,lang=sys.argv[1:4]
b=json.load(open(G+'base.json')); n=json.load(open(f'{G}m16-{tag}-{lvl}.json'))
rows=[]
for k,v in b.items():
    nm,l,o=k.rsplit('/',2)
    if l!=lang or o!=lvl or k not in n or 'instructions' not in n[k] or 'instructions' not in v: continue
    r=n[k]; rows.append((nm,*(r[m]/max(1,v[m]) for m in ('instructions','clocks','code_bytes'))))
for nm,i,c,b_ in sorted(rows,key=lambda x:x[2]): print(f'{nm:18} ins {i:.2f} clk {c:.2f} code {b_:.2f}')
