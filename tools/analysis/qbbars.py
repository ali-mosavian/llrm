#!/usr/bin/env python3
"""QB_TREE=<QB checkout> [QB_EXTRA=rel/a.c:rel/b.c] qbbars.py ARCH LEVEL DIST: QB src/core .text per file: ours (DIST/llrm-cc) vs gcc vs the pin's clang; worst by ours - best(gcc,clang)."""
import sys,os,glob,subprocess,tempfile,shutil
from concurrent.futures import ThreadPoolExecutor
A,L,D=sys.argv[1:4]; T=os.environ['QB_TREE']; b=os.environ.get('QB_SCRATCH',tempfile.gettempdir())
m,tgt=('-m64','x86_64-linux') if A=='m64' else ('-m32','x86_32-linux'); extra=['-isystem','/usr/include/x86_64-linux-gnu'] if A=='m32' else []
PIN_BIN=os.environ.get('PIN_BIN') or sys.exit('qbbars: set PIN_BIN to the pin build bin directory (clang)')
CLANG=os.path.join(PIN_BIN,'clang')
GCC=os.environ.get('QB_GCC','gcc-15')  # the baselines' gcc (15.2); plain `gcc` is the distro's 13.4
sys.path.insert(0,os.path.join(os.path.dirname(os.path.abspath(__file__)),'..'))
from external_flags import flags as bar_flags  # the one flag set every external bar is built with
def gcc_major():
    try: return int(subprocess.run([GCC,'-dumpversion'],capture_output=True,text=True).stdout.split('.')[0])
    except (ValueError,OSError): return None
RELOC=('-fPIE',) if A=='m64' else ('-fno-pic','-fno-pie')
CPU=['-march=i486'] if A=='m32' else []  # ours is a 486 (no SSE); a bar left at the distro's i686 default builds SSE  # what ours emits at the target; every column gets the same
BASE=['-D_DEFAULT_SOURCE',f'-{L}',m,*CPU,*extra,'-fno-unwind-tables','-D_FORTIFY_SOURCE=0','-w','-DARENA_AT_ZERO=0','-DPFIL_PATH=4096','-DEXEC_COUNT','-Isrc/plat/host/include','-Isrc','-Isrc/core']
CF=['-std=c23',*BASE,*bar_flags('x86',RELOC)]
GCC_CF=['-std=c2x',*BASE,*bar_flags('x86',RELOC,gcc_major())]
def sizes(o,prefixes):
    r=subprocess.run(['size','-A',o],capture_output=True,text=True).stdout
    return sum(int(l.split()[1]) for l in r.splitlines() if l.split() and any(l.split()[0]==p or l.split()[0].startswith(p+'.') for p in prefixes))
def text(o): return sizes(o,('.text',))
def whole(o):
    """.text plus the tables in .rodata*/.data* (no .bss): ours puts jump tables in .data, gcc and clang in .rodata, so .text alone mis-ranks."""
    return sizes(o,('.text','.rodata','.data'))
def one(f):
    out={}
    t=tempfile.mkdtemp(dir=b)
    try:
        for v,cmd,env in (('ours',[f'{D}/llrm-cc',*CF,'-c',f,'-o',f'{t}/o.o'],dict(os.environ,LLRM_CC_TARGET=tgt,LLRM_LLC=f'{D}/llrm-llc')),
                          ('gcc',[GCC,*GCC_CF,'-c',f,'-o',f'{t}/g.o'],os.environ),
                          ('clang',[CLANG,*CF,'-c',f,'-o',f'{t}/c.o'],os.environ)):
            r=subprocess.run(cmd,cwd=T,env=env,capture_output=True)
            o=cmd[-1]
            made=r.returncode==0 and os.path.exists(o)
            out[v]=text(o) if made else None; out[v+'_whole']=whole(o) if made else None
    finally: shutil.rmtree(t,ignore_errors=True)
    return os.path.relpath(f,T+'/src/core'),out
files=sorted(glob.glob(T+'/src/core/**/*.c',recursive=True))
files+=[os.path.join(T,p) for p in os.environ.get('QB_EXTRA','').split(':') if p]  # modules outside src/core
with ThreadPoolExecutor(6) as ex: res=list(ex.map(one,files))
ok=[(n,o) for n,o in res if all(v is not None for v in o.values())]
print(f'{A} -{L}: {len(ok)}/{len(res)} files with all three; failed:',[n for n,o in res if not all(v is not None for v in o.values())])
rows=sorted(((o['ours_whole']-min(o['gcc_whole'],o['clang_whole']),n,o) for n,o in ok),reverse=True)
print('total .text ours',sum(o['ours'] for _,o in ok),'gcc',sum(o['gcc'] for _,o in ok),'clang',sum(o['clang'] for _,o in ok),'sum(best)',sum(min(o['gcc'],o['clang']) for _,o in ok))
print('total whole ours',sum(o['ours_whole'] for _,o in ok),'gcc',sum(o['gcc_whole'] for _,o in ok),'clang',sum(o['clang_whole'] for _,o in ok),'sum(best)',sum(min(o['gcc_whole'],o['clang_whole']) for _,o in ok))
for d,n,o in rows[:14]: print(f'{d:+5d} whole ({o["ours"]-min(o["gcc"],o["clang"]):+5d} text) {n} ours {o["ours"]}/{o["ours_whole"]} gcc {o["gcc"]}/{o["gcc_whole"]} clang {o["clang"]}/{o["clang_whole"]}  (text/whole)')
print('worse',sum(1 for d,_,_ in rows if d>0),'of',len(rows),'sum worse',sum(d for d,_,_ in rows if d>0))
import json
if os.environ.get('QBJSON'): json.dump({n:o for n,o in res},open(os.environ['QBJSON'],'w'))
