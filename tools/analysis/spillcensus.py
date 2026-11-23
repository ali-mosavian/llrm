#!/usr/bin/env python3
"""QB_TREE=<QB checkout> spillcensus.py ARCH LEVEL DIST: per QB core module, stack-slot stores+reloads (objdump, same regex both sides), ours vs gcc, vs .text gap."""
import sys,os,glob,subprocess,tempfile,shutil,re
from concurrent.futures import ThreadPoolExecutor
A,L,D=sys.argv[1:4]; T=os.environ['QB_TREE']; b=os.environ.get('QB_SCRATCH',tempfile.gettempdir())
m,tgt,sp=('-m64','x86_64-linux','%rsp') if A=='m64' else ('-m32','x86_32-linux','%esp'); extra=['-isystem','/usr/include/x86_64-linux-gnu'] if A=='m32' else []
GCC=os.environ.get('QB_GCC','gcc-15')  # the baselines' gcc; plain `gcc` is the distro's 13.4
sys.path.insert(0,os.path.join(os.path.dirname(os.path.abspath(__file__)),'..'))
from external_flags import flags as bar_flags  # the one flag set every external bar is built with
major=None
try: major=int(subprocess.run([GCC,'-dumpversion'],capture_output=True,text=True).stdout.split('.')[0])
except (ValueError,OSError): pass
RELOC=('-fPIE',) if A=='m64' else ('-fno-pic','-fno-pie')
CPU=['-march=i486'] if A=='m32' else []
BASE=['-D_DEFAULT_SOURCE',f'-{L}',m,*CPU,*extra,'-fno-unwind-tables','-D_FORTIFY_SOURCE=0','-w','-DARENA_AT_ZERO=0','-DPFIL_PATH=4096','-DEXEC_COUNT','-Isrc/plat/host/include','-Isrc','-Isrc/core']
CF=[*BASE,*bar_flags('x86',RELOC)]
GCC_CF=['-D__cdecl=',*BASE,*bar_flags('x86',RELOC,major)]
slot=re.compile(r'\(%[re]sp\)')
def count(o):
    """(stores, reloads) of spill slots: an %rsp-relative slot is a spill only if every access to its offset in the function is a plain mov of a register and no lea takes its address or lies within 128 B below it (an aggregate)."""
    funcs={}; cur=None
    for l in subprocess.run(['objdump','-d','--no-show-raw-insn',o],capture_output=True,text=True).stdout.splitlines():
        h=re.match(r'^[0-9a-f]+ <(.*)>:',l)
        if h: cur=funcs.setdefault(re.sub(r'\.(isra|part|constprop|cold)(\.\d+)?','',h.group(1)),[]); continue
        p=l.split('\t')
        if cur is None or len(p)<2: continue
        m=re.match(r'(\w+)\s+(.*)',p[-1])
        if m: cur.append((m.group(1),m.group(2)))
    st=ld=0; rsp=re.compile(r'(-?0x[0-9a-f]+|)\(%rsp\)')
    for ins in funcs.values():
        bad=set(); acc=[]
        for op,args in ins:
            if op in('push','pop') or (op in('sub','add') and args.endswith(',%rsp')): continue
            m=rsp.search(args)
            if not m:
                if re.search(r'(?<!\()%rsp\b(?!\))',args) and op!='lea': bad.add('0x0')  # mov %rsp,%rdi
                continue
            off=m.group(1) or '0x0'
            if op=='lea' or not op.startswith('mov') or op.startswith(('movs','movz')) and False: bad.add(off); continue
            src,dst=args.rsplit(',',1) if ',' in args else (args,'')
            other=dst if rsp.search(src) else src
            if other.startswith('$') or not other.startswith('%') or '(' in other: bad.add(off); continue
            acc.append((off,bool(rsp.search(dst))))
        taken=[int(x,16) for x in bad]
        for off,is_store in acc:
            o=int(off,16)
            if off in bad or any(t<=o<t+128 for t in taken): continue
            if is_store: st+=1
            else: ld+=1
    return st,ld
def text(o):
    r=subprocess.run(['size','-A',o],capture_output=True,text=True).stdout
    return sum(int(l.split()[1]) for l in r.splitlines() if l.split() and (l.split()[0]=='.text' or l.split()[0].startswith('.text.')))
def one(f):
    t=tempfile.mkdtemp(dir=b); out={}
    try:
        for v,cmd,env in (('ours',[f'{D}/llrm-cc','-std=c23',*CF,'-c',f,'-o',f'{t}/o.o'],dict(os.environ,LLRM_CC_TARGET=tgt,LLRM_LLC=f'{D}/llrm-llc')),
                          ('gcc',[GCC,'-std=c2x',*GCC_CF,'-c',f,'-o',f'{t}/g.o'],os.environ)):
            r=subprocess.run(cmd,cwd=T,env=env,capture_output=True); o=cmd[-1]
            out[v]=(text(o),*count(o)) if r.returncode==0 and os.path.exists(o) else None
    finally: shutil.rmtree(t,ignore_errors=True)
    return os.path.relpath(f,T+'/src/core'),out
files=sorted(glob.glob(T+'/src/core/**/*.c',recursive=True))
with ThreadPoolExecutor(6) as ex: res=[r for r in ex.map(one,files) if all(r[1].values())]
worse=[(n,o) for n,o in res if o['ours'][0]>o['gcc'][0]]
print(f'{A} -{L}: {len(res)} modules, {len(worse)} worse than gcc')
more=0; expl=0; gap=0
for n,o in sorted(worse,key=lambda x:x[1]['gcc'][0]-x[1]['ours'][0]):
    g=o['ours'][0]-o['gcc'][0]; ops=sum(o['ours'][1:])-sum(o['gcc'][1:]); gap+=g
    if ops>0: more+=1; expl+=min(g,ops*4.5)
    print(f'{g:+5d} B  spill ops ours {sum(o["ours"][1:])} (st {o["ours"][1]} ld {o["ours"][2]}) gcc {sum(o["gcc"][1:])}  diff {ops:+d}  {n}')
print(f'worse modules with more spill ops than gcc: {more}/{len(worse)}; gap {gap} B, explained at 4.5 B/op (capped at the gap): {expl:.0f} B')
