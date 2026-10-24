"""QCport modules' emitted bytes (OMF LEDATA) at each level for BASE_LLRM and MAIN_LLRM.

--expect-base-sha / --expect-sha name the commits the two builds must come from, --allow-dirty accepts a tree with changes (tools/stamp.py)."""
import subprocess,sys,os,json
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
Q=Path(os.environ.get('QCPORT_SRC',os.path.expanduser('~/scratch/qcport/src'))); INC=os.environ.get('QCPORT_INC',os.path.expanduser('~/scratch/qctc/inc'))
MODS=("host","render","model","game","sound","ui")
inc=[f for d in (*MODS,"qgl") for f in ("-I",str(Q/d))]+["-I",INC]
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import stamp
flags,named,allow=stamp.options(sys.argv[1:])
bins={'base':os.environ['BASE_LLRM'],'main':os.environ['MAIN_LLRM']}
stamp.check(bins['base'],named['base_sha'],allow,'base'); stamp.check(bins['main'],named['sha'],allow,'main')
jobs=[(n,b,s,l) for n,b in bins.items() for d in MODS for s in sorted((Q/d).glob("*.c")) for l in ("-O1","-O2","-O3","-Os")]
def emitted(path):
    """Bytes of LEDATA in an OMF object: code and initialised data as emitted."""
    d=open(path,'rb').read(); i=0; total=0
    while i<len(d):
        t=d[i]; n=d[i+1]|d[i+2]<<8
        if t in (0xA0,0xA1):
            body=d[i+3:i+3+n-1]; k=1 if body[0]<0x80 else 2
            total+=len(body)-k-(4 if t==0xA1 else 2)
        i+=3+n
    return total
def run(j):
    n,b,s,l=j; o=f"/tmp/rows-qc-{n}-{s.stem}{l}.o"
    r=subprocess.run([b,l,*flags,*inc,"-o",o,str(s)],capture_output=True,text=True)
    if r.returncode: return (n,s.stem,l,None)
    t=emitted(o); os.unlink(o); return (n,s.stem,l,t)
with ThreadPoolExecutor(24) as p: res=list(p.map(run,jobs))
json.dump(res,open(os.path.join(os.environ.get('ROWS_WORK',os.path.expanduser('~/scratch/rows')),'qcport.json'),'w'))
failed={n:sum(1 for r in res if r[0]==n and r[3] is None) for n in bins}
print(len(res),"fail",sum(failed.values()))
if any(failed.values()): raise SystemExit(f"compiles failed {failed} of {len(res)//len(bins)} each: the report is void (a dist whose headers are gone fails every one)")
