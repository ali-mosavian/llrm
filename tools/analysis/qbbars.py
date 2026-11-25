#!/usr/bin/env python3
"""QB_TREE=<QB checkout> qbbars.py ARCH LEVEL DIST: per-module bytes (code = every executable section, and whole = code+.rodata*+.data*) of every module the QB product build compiles, ours (DIST/llrm-cc) vs gcc-15 vs the pin's clang.
The module set and each module's own -D/-I/-isystem are read from `make -n -B $QB_MAKE_TARGET` (default build/hostperf/basic, the perf build: no EXEC_COUNT) in QB_TREE; the shared external flags, -march=i486 at m32 and the level are appended for all three."""
import sys,os,re,shlex,subprocess,tempfile,shutil
from concurrent.futures import ThreadPoolExecutor
A,L,D=sys.argv[1:4]; T=os.environ['QB_TREE']; b=os.environ.get('QB_SCRATCH',tempfile.gettempdir())
m,tgt=('-m64','x86_64-linux') if A=='m64' else ('-m32','x86_32-linux'); extra=['-isystem','/usr/include/x86_64-linux-gnu'] if A=='m32' else []
PIN_BIN=os.environ.get('PIN_BIN') or sys.exit('qbbars: set PIN_BIN to the pin build bin directory (clang)')
CLANG=os.path.join(PIN_BIN,'clang')
GCC=os.environ.get('QB_GCC','gcc-15')  # the baselines' gcc (15.2); plain `gcc` is the distro's 13.4
sys.path.insert(0,os.path.join(os.path.dirname(os.path.abspath(__file__)),'..'))
import qbrow
from external_flags import flags as bar_flags  # the one flag set every external bar is built with
def gcc_major():
    try: return int(subprocess.run([GCC,'-dumpversion'],capture_output=True,text=True).stdout.split('.')[0])
    except (ValueError,OSError): return None
RELOC=('-fPIE',) if A=='m64' else ('-fno-pic','-fno-pie')
CPU=['-march=i486'] if A=='m32' else []  # ours is a 486 (no SSE); a bar left at the distro's i686 default builds SSE  # what ours emits at the target; every column gets the same
BASE=[f'-{L}',m,*CPU,*extra,'-fno-unwind-tables','-w']
CF=['-std=c23',*BASE,*bar_flags('x86',RELOC)]
FAST=['-fexcess-precision=fast'] if A=='m32' else []  # pfpu.c's constexpr doubles do not compile for gcc -m32 under x87 excess precision
GCC_CF=['-std=c2x','-D__cdecl=',*FAST,*BASE,*bar_flags('x86',RELOC,gcc_major())]
def product_modules():
    """(source, its own -D/-I/-isystem tokens) of every compile in the product's build, from `make -n -B`."""
    made=subprocess.run(['make','-n','-B',os.environ.get('QB_MAKE_TARGET','build/hostperf/basic')],cwd=T,capture_output=True,text=True).stdout
    mods=[]
    for line in made.splitlines():
        try: toks=shlex.split(line)
        except ValueError: continue
        if '-c' not in toks[:-1]: continue
        keep,i=[],1
        while i<len(toks):
            if toks[i] in ('-isystem','-I') and i+1<len(toks): keep+=toks[i:i+2]; i+=2; continue
            if toks[i].startswith(('-D','-I','-isystem')): keep.append(toks[i])
            i+=1
        mods.append((toks[toks.index('-c')+1],keep))
    return mods
CF=['-std=c23',*BASE,*bar_flags('x86',RELOC)]
FAST=['-fexcess-precision=fast'] if A=='m32' else []  # pfpu.c's constexpr doubles do not compile for gcc -m32 under x87 excess precision
GCC_CF=['-std=c2x','-D__cdecl=',*FAST,*BASE,*bar_flags('x86',RELOC,gcc_major())]
def sizes(o,prefixes):
    r=subprocess.run(['size','-A',o],capture_output=True,text=True).stdout
    return sum(int(l.split()[1]) for l in r.splitlines() if l.split() and any(l.split()[0]==p or l.split()[0].startswith(p+'.') for p in prefixes))
def executable(o):
    """Bytes of every section the object marks executable (SHF_EXECINSTR): .text, .text.* (unlikely, startup, hot, exit), .init, .fini and any named one, the same on every side. A name prefix misses a section a compiler puts code in under another name."""
    r=subprocess.run(['readelf','-SW',o],capture_output=True,text=True).stdout
    total=0
    for l in r.splitlines():
        m=re.match(r'\s*\[\s*\d+\]\s+(\S+)\s+\S+\s+\S+\s+\S+\s+([0-9a-f]+)\s+\S+\s+(\S*)',l)
        if m and 'X' in m.group(3): total+=int(m.group(2),16)
    return total
def text(o): return executable(o)
def whole(o):
    """The executable sections plus the tables in .rodata*/.data* (no .bss): ours puts jump tables in .data, gcc and clang in .rodata, so code alone mis-ranks."""
    return executable(o)+sizes(o,('.rodata','.data'))
def one(mod):
    f,own=mod
    out={}
    t=tempfile.mkdtemp(dir=b)
    try:
        for v,cmd,env in (('ours',[f'{D}/llrm-cc',*own,*CF,'-c',f,'-o',f'{t}/o.o'],dict(os.environ,LLRM_CC_TARGET=tgt,LLRM_LLC=f'{D}/llrm-llc')),
                          ('gcc',[GCC,*own,*GCC_CF,'-c',f,'-o',f'{t}/g.o'],os.environ),
                          ('clang',[CLANG,*own,*CF,'-c',f,'-o',f'{t}/c.o'],os.environ)):
            r=subprocess.run(cmd,cwd=T,env=env,capture_output=True)
            o=cmd[-1]
            made=r.returncode==0 and os.path.exists(o)
            out[v]=text(o) if made else None; out[v+'_whole']=whole(o) if made else None
    finally: shutil.rmtree(t,ignore_errors=True)
    return f,out
mods=product_modules()
qbrow.require_hosted(f'{D}/llrm-cc',tgt,{'LLRM_LLC':f'{D}/llrm-llc'})
if not mods: sys.exit('qbbars: `make -n -B` named no compile in '+T)
with ThreadPoolExecutor(6) as ex: res=list(ex.map(one,mods))
ok=[(n,o) for n,o in res if all(v is not None for v in o.values())]
print(f'{A} -{L}: {len(ok)}/{len(res)} files with all three; failed:',[n for n,o in res if not all(v is not None for v in o.values())])
rows=sorted(((o['ours_whole']-min(o['gcc_whole'],o['clang_whole']),n,o) for n,o in ok),reverse=True)
print('total .text ours',sum(o['ours'] for _,o in ok),'gcc',sum(o['gcc'] for _,o in ok),'clang',sum(o['clang'] for _,o in ok),'sum(best)',sum(min(o['gcc'],o['clang']) for _,o in ok))
print('total whole ours',sum(o['ours_whole'] for _,o in ok),'gcc',sum(o['gcc_whole'] for _,o in ok),'clang',sum(o['clang_whole'] for _,o in ok),'sum(best)',sum(min(o['gcc_whole'],o['clang_whole']) for _,o in ok))
for d,n,o in rows[:14]: print(f'{d:+5d} whole ({o["ours"]-min(o["gcc"],o["clang"]):+5d} text) {n} ours {o["ours"]}/{o["ours_whole"]} gcc {o["gcc"]}/{o["gcc_whole"]} clang {o["clang"]}/{o["clang_whole"]}  (text/whole)')
print('worse',sum(1 for d,_,_ in rows if d>0),'of',len(rows),'sum worse',sum(d for d,_,_ in rows if d>0))
import json
if os.environ.get('QBJSON'): json.dump({n:o for n,o in res},open(os.environ['QBJSON'],'w'))
