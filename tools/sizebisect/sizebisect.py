"""Machine instruction counts per stage of two builds' compile of one file (ISEL_DUMP): where a size change enters.

    tools/sizebisect/sizebisect.py BASE_LLRM NEW_LLRM LEVEL SRC [llrm-c args...]
"""
import os,re,sys,glob,subprocess
base,new,lvl,src=sys.argv[1:5]; extra=sys.argv[5:]
def dump(llrm,tag):
    d=os.path.abspath(f"sb-{tag}"); subprocess.run(["rm","-rf",d]); os.makedirs(d)
    env=dict(os.environ,LLRM_MIR_STAGES=d,ISEL_DUMP="1")
    r=subprocess.run([llrm,lvl,*extra,"-S","-o",f"{d}/out.s",src],env=env,capture_output=True,text=True)
    return d,r.stdout+r.stderr
def count(text):
    return sum(1 for l in text.split("\n") if re.match(r"\s+(%\S+ = |store |br |ret |call |switch |unreachable)",l))
def order(f):
    parts=re.split(r"[/-]",f); return [int(x) if x.isdigit() else x for x in parts[:3]]
db,tb=dump(base,"base"); dn,tn=dump(new,"new")
files=sorted({os.path.relpath(p,db) for p in glob.glob(db+"/**/*.ll",recursive=True)},key=lambda f:(f.count("/"),f))
print("MIR stages: file  base-insns new-insns")
prev=None
for f in files:
    a=open(f"{db}/{f}").read(); 
    p=f"{dn}/{f}"
    if not os.path.exists(p): print(f,"missing in new"); continue
    b=open(p).read(); ca,cb=count(a),count(b)
    if (ca,cb)!=prev and ca!=cb: print(f"{f:50} {ca:6} {cb:6} {cb-ca:+d}")
    prev=(ca,cb)
# machine stages from ISEL_DUMP text: count lines per '=== name' block
def blocks(t):
    parts=re.split(r"^=== (.*)$",t,flags=re.M); return list(zip(parts[1::2],parts[2::2]))
ba,bb=blocks(tb),blocks(tn)
print("machine stages: name base new")
for (n1,x),(n2,y) in zip(ba,bb):
    ca=sum(1 for l in x.split("\n") if re.match(r"\s+0x[0-9a-f]+ ",l)); cb=sum(1 for l in y.split("\n") if re.match(r"\s+0x[0-9a-f]+ ",l))
    if ca!=cb: print(f"{n1:28} {ca:6} {cb:6} {cb-ca:+d}")
sa=open(f"{db}/out.s").read().count("\n"); sb=open(f"{dn}/out.s").read().count("\n"); print("asm lines",sa,sb)
