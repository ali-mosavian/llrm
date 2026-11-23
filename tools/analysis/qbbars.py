#!/usr/bin/env python3
"""QB_TREE=<QB checkout> qbbars.py ARCH LEVEL DIST: QB src/core .text per file: ours (DIST/llrm-cc) vs gcc vs the pin's clang; worst by ours - best(gcc,clang)."""
import sys,os,glob,subprocess,tempfile,shutil
from concurrent.futures import ThreadPoolExecutor
A,L,D=sys.argv[1:4]; T=os.environ['QB_TREE']; b=os.environ.get('QB_SCRATCH',tempfile.gettempdir())
m,tgt=('-m64','x86_64-linux') if A=='m64' else ('-m32','x86_32-linux'); extra=['-isystem','/usr/include/x86_64-linux-gnu'] if A=='m32' else []
PIN_BIN=os.environ.get('PIN_BIN') or sys.exit('qbbars: set PIN_BIN to the pin build bin directory (clang)')
CLANG=os.path.join(PIN_BIN,'clang')
CF=['-std=c23','-D_DEFAULT_SOURCE',f'-{L}',m,*extra,'-fno-asynchronous-unwind-tables','-fno-unwind-tables','-fno-stack-protector','-fcf-protection=none','-U_FORTIFY_SOURCE','-D_FORTIFY_SOURCE=0','-fno-pic','-w','-DARENA_AT_ZERO=0','-DPFIL_PATH=4096','-DEXEC_COUNT','-Isrc/plat/host/include','-Isrc','-Isrc/core']
def text(o):
    r=subprocess.run(['size','-A',o],capture_output=True,text=True).stdout
    return sum(int(l.split()[1]) for l in r.splitlines() if l.split() and l.split()[0] in('.text',) or (l.split() and l.split()[0].startswith('.text.')))
def one(f):
    out={}
    t=tempfile.mkdtemp(dir=b)
    try:
        for v,cmd,env in (('ours',[f'{D}/llrm-cc',*CF,'-c',f,'-o',f'{t}/o.o'],dict(os.environ,LLRM_CC_TARGET=tgt,LLRM_LLC=f'{D}/llrm-llc')),
                          ('gcc',['gcc',*[('-std=c2x' if x=='-std=c23' else x) for x in CF],'-c',f,'-o',f'{t}/g.o'],os.environ),
                          ('clang',[CLANG,*CF,'-c',f,'-o',f'{t}/c.o'],os.environ)):
            r=subprocess.run(cmd,cwd=T,env=env,capture_output=True)
            o=cmd[-1]
            out[v]=text(o) if r.returncode==0 and os.path.exists(o) else None
    finally: shutil.rmtree(t,ignore_errors=True)
    return os.path.relpath(f,T+'/src/core'),out
files=sorted(glob.glob(T+'/src/core/**/*.c',recursive=True))
with ThreadPoolExecutor(6) as ex: res=list(ex.map(one,files))
ok=[(n,o) for n,o in res if all(v is not None for v in o.values())]
print(f'{A} -{L}: {len(ok)}/{len(res)} files with all three; failed:',[n for n,o in res if not all(v is not None for v in o.values())])
rows=sorted(((o['ours']-min(o['gcc'],o['clang']),n,o) for n,o in ok),reverse=True)
print('total ours',sum(o['ours'] for _,o in ok),'gcc',sum(o['gcc'] for _,o in ok),'clang',sum(o['clang'] for _,o in ok),'sum(best)',sum(min(o['gcc'],o['clang']) for _,o in ok))
for d,n,o in rows[:14]: print(f'{d:+5d} {n} ours {o["ours"]} gcc {o["gcc"]} clang {o["clang"]}')
print('worse',sum(1 for d,_,_ in rows if d>0),'of',len(rows),'sum worse',sum(d for d,_,_ in rows if d>0))
import json
if os.environ.get('QBJSON'): json.dump({n:o for n,o in res},open(os.environ['QBJSON'],'w'))
