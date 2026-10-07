import json,collections,re,sys
G='/home/alim/scratch/llvm-spike/gate/'
d=json.load(open(G+sys.argv[1]))
c=collections.Counter(); ex={}
for k,v in d.items():
    if 'error' in v:
        e=v['error']
        m=re.search(r'(error: [^\n]*|llrm-[a-z]+: [^\n]*|panicked[^\n]*)',e,re.S)
        key=(k.split('/')[1], re.sub(r'<stdin>:\d+:\d+','<stdin>',(m.group(1) if m else e))[:120]); c[key]+=1; ex.setdefault(key,k)
for k,n in c.most_common(40): print(n,k,ex[k])
print('ok',sum(1 for v in d.values() if 'error' not in v), len(d))
