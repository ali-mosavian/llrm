"""The spill model's forecast beside the allocator's spill code, per function of every bench program, on both targets.

Run from the repo root after `cargo build --release`: `python3 tools/pressure.py`. A price is clocks per entry.
"""
import json
import math
import glob
import os
import re
import subprocess
rows=[]
for tgt,flags in (("m32",["-m32","-march=i486"]),("m16",[])):
    for src in sorted(glob.glob("bench/*/*.c")):
        p=os.path.basename(src)[:-2]
        if p in("readme","parity","huge","textfill","grep"): continue
        r=subprocess.run(["./target/release/llrm-c",*flags,"-O2","-fno-inline-functions","-S","-o","/dev/null",src],capture_output=True,text=True,env={**os.environ,"LLRM_DEBUG":"pressure"})
        for l in r.stderr.splitlines():
            m=re.match(r"\[pressure\] (\S+) forecast peak (\d+) spilled (\d+) price (\d+); allocator (\d+) reloads, (\d+) stores, (\d+) remats, price (\d+)",l)
            if m: rows.append((tgt,p,m.group(1),*map(int,m.groups()[1:])))
for tgt in ("m32","m16"):
    R=[r for r in rows if r[0]==tgt]
    both0=sum(1 for r in R if r[5]==0 and r[9]==0)
    fn=[r for r in R if r[5]>0 and r[9]==0]   # forecast spill, none actual
    fp=[r for r in R if r[5]==0 and r[9]>0]
    both=[r for r in R if r[5]>0 and r[9]>0]
    print(tgt,"functions",len(R),"both none",both0,"forecast only",len(fn),"allocator only",len(fp),"both",len(both))
    if both:
        lr=[math.log(r[5]/r[9]) for r in both]; print("  geomean forecast/actual price where both spill: %.2f"%math.exp(sum(lr)/len(lr)))
    print("  largest allocator-only:",sorted(fp,key=lambda r:-r[9])[:5])
    print("  largest forecast-only:",sorted(fn,key=lambda r:-r[5])[:5])
