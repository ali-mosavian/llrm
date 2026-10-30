"""Same-IR isel quality: the pin's -O2 IR (clang -O2 -S -emit-llvm, one .ir.ll per program in DIR) through
llrm-arm64-llc --isel=dag, --isel=hand and the pin's llc -O2; prints instruction, memory-op and
stack-memory-op totals per function. Needs LLRM_PIN_CLANG. `python3 tools/arm64_quality/same_ir.py DIR`."""
import sys,glob,os,subprocess,collections,re
sys.argv=[sys.argv[0],sys.argv[1]]
S=sys.argv[1]
src=open(os.path.dirname(os.path.abspath(__file__))+'/hand_vs_dag_vs_pin.py').read().split("tot=collections.Counter()")[0]
exec(src)  # quality.py's funcs()
llc=os.environ['HOME']+'/work/other/llvm-pin/build/bin/llc'
tot=collections.Counter(); rows=[]; skipped=[]
for ir in sorted(glob.glob(S+'/*.ir.ll')):
    n=os.path.basename(ir)[:-6]
    r=lambda args,env: subprocess.run(args,capture_output=True,text=True,env={**os.environ,**env}).returncode==0
    if not (r(['target/debug/llrm-arm64-llc','-S','--isel=dag','-o',f'{S}/{n}.q.dag.s',ir],{}) and r(['target/debug/llrm-arm64-llc','-S','--isel=hand','-o',f'{S}/{n}.q.old.s',ir],{}) and r([llc,'-O2','-mtriple=arm64-apple-macosx','-o',f'{S}/{n}.q.pin.s',ir],{})):
        skipped.append(n); continue
    d,o,p=funcs(f'{S}/{n}.q.dag.s'),funcs(f'{S}/{n}.q.old.s'),funcs(f'{S}/{n}.q.pin.s')
    for fn in d:
        if fn in o and fn in p:
            rows.append((n,fn,d[fn],o[fn],p[fn]))
            for k,v in (('dag',d[fn]),('old',o[fn]),('pin',p[fn])):
                for i,c in enumerate(('insn','mem','sp')): tot[(k,c)]+=v[i]
print('programs skipped (dag refused):',skipped)
print('functions compared:',len(rows))
for c in ('insn','mem','sp'): print(c,'dag',tot[('dag',c)],'hand',tot[('old',c)],'pin',tot[('pin',c)])
worse=[r for r in rows if r[2][0]>r[4][0]]
print('dag more insns than pin:',len(worse),' fewer:',len([r for r in rows if r[2][0]<r[4][0]]),' equal:',len([r for r in rows if r[2][0]==r[4][0]]))
for r in sorted(worse,key=lambda r:r[4][0]-r[2][0])[:12]: print(r[0],r[1],'dag',r[2],'hand',r[3],'pin',r[4])
