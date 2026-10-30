import re,sys,subprocess,glob,os,collections
S=sys.argv[1]; root='crates/target/llrm-arm64/tests/programs'
pin=os.environ['LLRM_PIN_CLANG']; cc='target/debug/llrm-cc'
MEM=re.compile(r'^(ldr|str|ldp|stp|ldur|stur|ldrb|strb|ldrh|strh|ldrsw|ldrsb|ldrsh|ld[1-4]|st[1-4])\b')
def funcs(path):
    out={}; cur=None
    for line in open(path):
        l=line.split(';')[0].rstrip()
        m=re.match(r'^(_?[A-Za-z_][\w$.]*):',l)
        if m and not m.group(1).startswith(('L','l_','ltmp')): cur=m.group(1); out[cur]=[0,0,0]; continue
        if cur is None: continue
        t=l.strip()
        if not t or t.startswith(('.','L','_')) or t.endswith(':'): continue
        op=t.split()[0]
        out[cur][0]+=1
        if MEM.match(op):
            out[cur][1]+=1
            if re.search(r'\[sp[,\]]',t): out[cur][2]+=1
    return out
tot=collections.Counter(); rows=[]
for f in sorted(glob.glob(root+'/*.c')):
    n=os.path.basename(f)[:-2]
    def run(cmd,env,out):
        r=subprocess.run(cmd+['-S','-O2','-target','arm64-apple-macosx',f,'-o',out],capture_output=True,text=True,env={**os.environ,**env})
        return r.returncode==0
    ok=run([cc],{'LLRM_ISEL_SHADOW':'1'},f'{S}/{n}.dag.s') and run([cc],{},f'{S}/{n}.old.s') and run([pin],{},f'{S}/{n}.pin.s')
    if not ok: print(n,'a build failed'); continue
    d,o,p=funcs(f'{S}/{n}.dag.s'),funcs(f'{S}/{n}.old.s'),funcs(f'{S}/{n}.pin.s')
    for fn in d:
        if fn in o and fn in p and d[fn]!=o[fn]:
            rows.append((n,fn,d[fn],o[fn],p[fn]))
            for k,v in (('dag',d[fn]),('old',o[fn]),('pin',p[fn])):
                for i,c in enumerate(('insn','mem','sp')): tot[(k,c)]+=v[i]
print('dag-selected functions (differ from hand):',len(rows))
for c in ('insn','mem','sp'): print(c,'dag',tot[('dag',c)],'hand',tot[('old',c)],'pin',tot[('pin',c)])
worse=[r for r in rows if r[2][0]>r[4][0]]
print('functions with more insns than the pin:',len(worse))
for r in sorted(worse,key=lambda r:r[4][0]-r[2][0])[:15]: print(r[0],r[1],'dag',r[2],'hand',r[3],'pin',r[4])
