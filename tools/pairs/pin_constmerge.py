"""The pin's constmerge and second globaldce over the QCport modules at -Os: llc object sizes with each removed (2026-10-23: constmerge +58 B of 399336 on 2 modules, globaldce 0)."""
import subprocess, glob, os, re, sys, shutil, tempfile
from concurrent.futures import ThreadPoolExecutor
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pin
H=os.path.expanduser("~"); L=f"{H}/scratch/sbp-e1dd29df450dd3161452a7fe4de399be02d384f7/dist/llrm-cc"  # a dist llrm-cc; CM_LLRM overrides
L=os.environ.get("CM_LLRM", L)
Q=f"{H}/scratch/qcport/src"; MODS=("host","render","model","game","sound","ui")
inc=[f for d in (*MODS,"qgl") for f in ("-I",f"{Q}/{d}")]+["-I",f"{H}/scratch/qctc/inc"]
of=pin.target("m16",optsize=True)
def tail_split(full):
    # the pin's module pipeline ends: ... globaldce, constmerge, cg-profile, rel-lookup-table-converter, lower-comment-string
    return full
def one(src):
    name=os.path.relpath(src,Q)
    d=tempfile.mkdtemp(prefix="cm-",dir=f"{H}/scratch")
    try:
        r=subprocess.run([L,"-c","-Os",*inc,"--dump",d+"/d","-o",d+"/o.obj",src],capture_output=True,text=True,env={**os.environ,"LLRM_CC_TARGET":"x86_16-wat"})
        f=d+"/d/09-pipeline.ll"
        if not os.path.exists(f): return name,None
        text=open(f).read().replace(" threeway","")
        full=pin.opt("default<O2>",text,of,extra=["-print-pipeline-passes","-disable-output"]).stdout.split("\n",1)[1].strip()
        # remove the trailing constmerge and the globaldce before it
        m=re.search(r"(.*),globaldce,constmerge(,cg-profile.*)$",full)
        if not m: return name,"nopipe"
        variants={"full":full,"no-constmerge":m.group(1)+",globaldce"+m.group(2),"no-globaldce2":m.group(1)+",constmerge"+m.group(2),"neither":m.group(1)+m.group(2)}
        out={}
        for k,pipe in variants.items():
            o=pin.opt(pipe,text,of)
            if o.returncode: return name,"opt:"+o.stderr[:80]
            rr=pin.llc(["-O2","-filetype=obj","-o",d+f"/{k}.o"],o.stdout,of)
            if rr.returncode: return name,"llc:"+rr.stderr[:80]
            sz=subprocess.run(["/usr/bin/size",d+f"/{k}.o"],capture_output=True,text=True).stdout.split("\n")[1].split()
            out[k]=(int(sz[0]),int(sz[1]))  # text, data
        return name,out
    finally:
        shutil.rmtree(d,ignore_errors=True)
srcs=sorted(s for dd in MODS for s in glob.glob(f"{Q}/{dd}/*.c"))
rows=[]
with ThreadPoolExecutor(12) as p:
    for name,out in p.map(one,srcs): rows.append((name,out))
bad=[r for r in rows if not isinstance(r[1],dict)]
print("modules",len(rows),"failed",len(bad),bad[:3])
tot={k:[0,0] for k in ("full","no-constmerge","no-globaldce2","neither")}
chg=[]
for name,out in rows:
    if not isinstance(out,dict): continue
    for k,(t,dd) in out.items(): tot[k][0]+=t; tot[k][1]+=dd
    if len({v for v in out.values()})>1: chg.append((name,out))
print("totals (text,data):",tot)
print("modules where a variant differs:",len(chg))
for name,out in sorted(chg,key=lambda r:-abs(r[1]['full'][1]-r[1]['no-constmerge'][1]))[:10]: print(name,out)
