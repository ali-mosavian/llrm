"""Compile time of each compiler, per program: best of 5 wall-clock runs, source to object, one process each."""
import json, os, subprocess, sys, time
from pathlib import Path
R = Path(__file__).resolve().parents[4]; O = Path(os.environ.get("VSGCC_WORK", Path.home() / "scratch/vsgcc-work"))
progs = [p for p in sorted(x.name for x in (R / "bench").iterdir()) if p not in ("readme.md", "parity", "huge", "textfill", "grep")]
F = "-m32 -march=i486 -fno-pic -fno-inline-functions -fno-stack-protector -fcf-protection=none -Dfar=".split()
def cmds(p):
    s = R / "bench" / p / f"{p}.c"; g = O / "b" / f"{p}.c"
    ll = [str(R / "target/release/llrm-c"), "--target", "x86-code32", "-march=i486", "-fno-inline-functions"]
    return {"llrm O2": ll + ["-O2", "-o", "/dev/null", str(s)], "llrm Os": ll + ["-Os", "-o", "/dev/null", str(s)],
            "gcc O2": ["gcc", *F, "-O2", "-c", "-o", "/dev/null", str(g)], "gcc Os": ["gcc", *F, "-Os", "-c", "-o", "/dev/null", str(g)],
            "clang O2": ["clang", *F, "-O2", "-c", "-o", "/dev/null", str(g)], "clang Os": ["clang", *F, "-Os", "-c", "-o", "/dev/null", str(g)]}
res = {}
for p in progs:
    for k, c in cmds(p).items():
        best = 1e9
        for _ in range(5):
            t = time.perf_counter(); subprocess.run(c, check=True, capture_output=True); best = min(best, time.perf_counter() - t)
        res.setdefault(p, {})[k] = best * 1000
json.dump(res, open(O / "ctime.json", "w"))
