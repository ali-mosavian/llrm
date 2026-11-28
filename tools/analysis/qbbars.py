#!/usr/bin/env python3
"""QB_TREE=<QB checkout> qbbars.py ARCH LEVEL DIST (ARCH: m64 m32 arm64 m68k-68000 m68k-68020): per-module bytes (code = every executable section, and whole = code+.rodata*+.data*) of every module the QB product build compiles, ours (DIST/llrm-cc) vs gcc-15 vs the pin's clang (m68k: gcc only).
The module set and each module's own -D/-I/-isystem are read from `make -n -B $QB_MAKE_TARGET` (default build/hostperf/basic, the perf build: no EXEC_COUNT) in QB_TREE; the shared external flags, -march=i486 at m32 and the level are appended for all three."""
import sys,os,re,shlex,subprocess,tempfile,shutil
from concurrent.futures import ThreadPoolExecutor
A,L,D=sys.argv[1:4]; T=os.environ['QB_TREE']; b=os.environ.get('QB_SCRATCH',tempfile.gettempdir())
sys.path.insert(0,os.path.join(os.path.dirname(os.path.abspath(__file__)),'..'))
import qbrow
import stamp
from external_flags import flags as bar_flags, gcc_float  # the one flag set every external bar is built with
# One row per arch. cc: -m/-march words every column gets; gcc: the baseline compiler (gcc 15.2; the distro's plain `gcc` is 13.4);
# gcc_cc: gcc-only words (float mode, excess precision, CPU); clang: the pin's clang target words, None = no clang bar; llc: the
# sysroot: the env var naming the target's glibc root (QB is a hosted program: arm64-glibc, not the freestanding arm64-linux row); llrm-*-llc in DIST that LLRM_LLC names, None = the one beside llrm-cc; std: gcc's dialect.
ARCHES={
 'm64':{'tgt':'x86_64-linux','family':'x86','cc':['-m64'],'reloc':('-fPIE',),'gcc':'gcc-15','gcc_cc':[],'extra':[],'clang':[],'llc':'llrm-llc','std':'c2x'},
 # ours is a 486 (no SSE): a bar left at the distro's i686 default builds SSE. pfpu.c's constexpr doubles do not compile for gcc -m32 under x87 excess precision.
 'm32':{'tgt':'x86_32-linux','family':'x86','cc':['-m32','-march=i486'],'reloc':('-fno-pic','-fno-pie'),'gcc':'gcc-15','gcc_cc':['-fexcess-precision=fast'],'extra':['-isystem','/usr/include/x86_64-linux-gnu'],'clang':[],'llc':'llrm-llc','std':'c2x'},
 'arm64':{'tgt':'arm64-glibc','sysroot':'LLRM_ARM64_SYSROOT','family':'arm64','cc':[],'reloc':('-fPIE',),'gcc':'aarch64-linux-gnu-gcc-15','gcc_cc':[],'extra':[],'clang':['--target=aarch64-linux-gnu'],'llc':'llrm-arm64-llc','std':'c2x'},
}
for cpu in ('68000','68020'):  # gcc-only bar: no clang for m68k
    ARCHES['m68k-'+cpu]={'tgt':'m68k-'+cpu,'family':'none','cc':[],'reloc':(),'gcc':'m68k-linux-gnu-gcc','gcc_cc':[f'-m{cpu}',*gcc_float('m68k')],'extra':[],'clang':None,'llc':None,'std':'gnu2x'}
if A not in ARCHES: sys.exit(f'qbbars: ARCH is one of {", ".join(ARCHES)}')
R=ARCHES[A]; tgt=R['tgt']
SYSROOT=os.environ.get(R['sysroot']) if R.get('sysroot') else None
if R.get('sysroot') and not SYSROOT: sys.exit(f"qbbars: set {R['sysroot']} to the unpacked {A} cross packages (usr/aarch64-linux-gnu/include under it; / for a system install)")
GCC=os.environ.get('QB_GCC',R['gcc'])
if R['clang'] is None: CLANG=None
else:
    PIN_BIN=(os.environ.get('PIN_BIN_'+A.upper()) or os.environ.get('PIN_BIN')) or sys.exit(f'qbbars: set PIN_BIN to the pin build bin directory (clang; PIN_BIN_{A.upper()} for a pin built for {A})')
    CLANG=os.path.join(PIN_BIN,'clang')
def gcc_major():
    try: return int(subprocess.run([GCC,'-dumpversion'],capture_output=True,text=True).stdout.split('.')[0])
    except (ValueError,OSError): return None
BASE=[f'-{L}',*R['cc'],*R['extra'],'-fno-unwind-tables','-w']
CF=['-std=c23',*BASE,*bar_flags(R['family'],R['reloc'])]
GCC_CF=[f"-std={R['std']}",'-D__cdecl=',*R['gcc_cc'],*BASE,*bar_flags(R['family'],R['reloc'],gcc_major())]
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
def clang_headers():
    """The arm64-glibc row's header order for the pin's clang, as targets.toml has it: no host headers (x86 glibc), clang's own, the cross glibc last (`-idirafter`) so a project's `-isystem` dir wins: a plain `-isystem` glibc put its error.h ahead of the project's and qbrt.c read as refused (b_inonerr undeclared)."""
    if not SYSROOT: return []
    res=subprocess.run([CLANG,'-print-resource-dir'],capture_output=True,text=True).stdout.strip()
    return ['-nostdinc','-isystem',res+'/include','-idirafter',SYSROOT+'/usr/aarch64-linux-gnu/include']
BARS=('gcc','clang') if CLANG else ('gcc',)
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
        ours_env=dict(os.environ,LLRM_CC_TARGET=tgt)
        if R['llc']: ours_env['LLRM_LLC']=f"{D}/{R['llc']}"
        cmds=[('ours',[f'{D}/llrm-cc',*own,*CF,'-c',f,'-o',f'{t}/o.o'],ours_env),('gcc',[GCC,*own,*GCC_CF,'-c',f,'-o',f'{t}/g.o'],os.environ)]
        if CLANG: cmds.append(('clang',[CLANG,*R['clang'],*clang_headers(),*own,*CF,'-c',f,'-o',f'{t}/c.o'],os.environ))
        for v,cmd,env in cmds:
            r=subprocess.run(cmd,cwd=T,env=env,capture_output=True)
            o=cmd[-1]
            made=r.returncode==0 and os.path.exists(o)
            out[v]=text(o) if made else None; out[v+'_whole']=whole(o) if made else None
    finally: shutil.rmtree(t,ignore_errors=True)
    return f,out
def best(o,k=''): return min(o[v+k] for v in BARS)
def report(res):
    """The report lines for per-module results [(file, {column: bytes})]: totals per column, the worst modules, the worse count."""
    ok=[(n,o) for n,o in res if all(v is not None for v in o.values())]
    cols=('ours',*BARS)
    lines=[f'{A} -{L}: {len(ok)}/{len(res)} files with all {('one','two','three')[len(cols)-1]}; failed: {[n for n,o in res if not all(v is not None for v in o.values())]}']
    rows=sorted(((o['ours_whole']-best(o,'_whole'),n,o) for n,o in ok),reverse=True)
    for k,label in (('','.text'),('_whole','whole')):
        lines.append(f'total {label} '+' '.join(f'{c} {sum(o[c+k] for _,o in ok)}' for c in cols)+f' sum(best) {sum(best(o,k) for _,o in ok)}')
    for d,n,o in rows[:14]:
        lines.append(f'{d:+5d} whole ({o["ours"]-best(o):+5d} text) {n} '+' '.join(f'{c} {o[c]}/{o[c+"_whole"]}' for c in cols)+'  (text/whole)')
    lines.append(f'worse {sum(1 for d,_,_ in rows if d>0)} of {len(rows)} sum worse {sum(d for d,_,_ in rows if d>0)}')
    return lines
if __name__=='__main__':
    mods=product_modules()
    stamp.require_named(D)  # DIST is the commit LLRM_EXPECT_SHA names, built clean
    qbrow.require_hosted(f'{D}/llrm-cc',tgt,{'LLRM_LLC':f"{D}/{R['llc']}"} if R['llc'] else {})
    if not mods: sys.exit('qbbars: `make -n -B` named no compile in '+T)
    with ThreadPoolExecutor(6) as ex: res=list(ex.map(one,mods))
    print('\n'.join(report(res)))
    import json
    if os.environ.get('QBJSON'): json.dump({n:o for n,o in res},open(os.environ['QBJSON'],'w'))
