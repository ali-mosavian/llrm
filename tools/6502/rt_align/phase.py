"""Judge-grid phase of the hot runtime loops (6502-RT-ALIGN). Run from the repo root: RT_S=mos_rt.s RT_LST=<ca65 -l listing of the runtime>
LLRM_CC=llrm-cc python3 tools/6502/rt_align/phase.py O2. sim65 charges +1 clk for a taken branch whose opcode page differs from the target page."""
import os, sys, re, json, tempfile, subprocess, collections
os.environ["OURS_RT"]=os.environ["RT_S"]
sys.path.insert(0,"tools/6502")
import ours_cr as oc
from pathlib import Path
CC=os.environ["LLRM_CC"]
# loops: routine -> [(branch opcode offset, target offset)] from the assembled listing, relative to the routine label
lst=open(os.environ["RT_LST"]).read().splitlines()
labels={};scope="";br=[]
for l in lst:
    m=re.match(r"^([0-9A-F]{6})r? \d+\s+((?:[0-9A-F]{2} ?)*)\s*(.*)$",l)
    if not m: continue
    a=int(m.group(1),16); by=m.group(2).split(); t=m.group(3).split(";")[0].strip()
    lm=re.match(r"^([A-Za-z_@][\w@]*):",t)
    if lm:
        n=lm.group(1)
        if not n.startswith("@"): scope=n
        labels[(scope,n)]=a; t=t[lm.end():].strip()
    bm=re.match(r"^(b(?:ne|eq|cc|cs|pl|mi|vc|vs))\s+(@?\w+)",t)
    if bm and by: br.append((scope,a,bm.group(2)))
HOT={"__mulsi3","__mulsi3_fast","udivmod32_fast_small","udivmod32_wide"}
loops={}
for sc,a,t in br:
    if sc in HOT and (sc,t) in labels and labels[(sc,t)]<a:
        loops.setdefault(sc,[]).append((a-labels[(sc,sc)],labels[(sc,t)]-labels[(sc,sc)],t))
pg=lambda x:x>>8
def cross(A,off): return [pg(A+off[0])!=pg(A+off[1]) for A in [A]]
progs=[x.strip() for x in open("tools/6502/pass82.txt")]
level=sys.argv[1]
rows=[]
W=tempfile.mkdtemp(prefix="ph-")
d_s=oc.driver()
for name in progs:
    src=oc.src_of(name); w=Path(tempfile.mkdtemp(prefix=name+"-",dir=W))
    (w/"d.s").write_text(d_s)
    c=oc.sh([CC,"--target=6502-sim65",f"-{level}","-S","-w",*oc.CORPUS_COMMON,*oc.program_flags(src),"-I","tools/6502/inc",str(src),"-o","o.s"],w)
    if c.returncode: continue
    asm=(w/"o.s").read_text(); rt=oc.reachable_runtime(asm)
    used=[h for h in HOT if re.search(r"^"+h+":",rt,re.M)]
    if not used: continue
    (w/"f.s").write_text(".autoimport +\n"+asm+rt+"".join(f".export {h}\n" for h in used))
    k=oc.sh(["cl65","-t","sim6502","-Ln","x.lbl","-o","x.bin","d.s","f.s"],w)
    if k.returncode: continue
    lbl=open(w/"x.lbl").read()
    for h in used:
        A=int(re.search(r"al ([0-9A-F]+) \."+h+r"\b",lbl).group(1),16)
        rows.append((name,h,A))
json.dump({"rows":rows,"loops":loops},open(f"phase-{level}.json","w"))
print(len(rows),"routine instances over",len({r[0] for r in rows}),"programs")
