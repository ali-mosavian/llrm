"""Premise for codegen row 42 (docs/working/early-machinelicm.md): on the same IR at m32, instructions of ours vs the pin's llc with and without -disable-machine-licm.

    mlicm.py LLRM_CC LEVEL(-O2|-Os) SRC_DIR [m32|m16]   (m16 compiles the QCport modules under ~/scratch/qcport/src)
"""
import os
import glob, re, shutil, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pin

OURS, LV, SRC = sys.argv[1], sys.argv[2], Path(sys.argv[3])
MODE = sys.argv[4] if len(sys.argv) > 4 else "m32"
of = pin.target(MODE, optsize=(LV == "-Os"))
Q = Path.home() / "scratch/qcport/src"
MODS = ("host", "render", "model", "game", "sound", "ui")
INC = [x for d in (*MODS, "qgl") for x in ("-I", str(Q / d))] + ["-I", str(Path.home() / "scratch/qctc/inc")] if MODE == "m16" else []
FLAGS = ["-m16"] if MODE == "m16" else ["-m32", "-mabi=sysv", "-march=i486"]
os.environ["LLRM_CC_TARGET"] = "x86_16-wat" if MODE == "m16" else "x86_32-wat"  # llrm-cc takes its target row from the environment
WORK = Path.home() / "scratch/w3-mlicm/tmp"
WORK.mkdir(exist_ok=True)


def count(asm):
    ins = [l for l in asm.splitlines() if re.match(r"\s+[a-z]", l) and not l.strip().startswith((".", "#", ";"))]
    return len(ins)


def loops(asm):
    """(instructions inside loop ranges counted once, summed over ranges): a range is a label to a later branch to it."""
    lines = [l.strip() for l in asm.splitlines() if l.strip() and (re.match(r"[\w.$@?]+:", l.strip()) or not l.strip().startswith((".", "#", ";", "//")))]
    pos, kinds = {}, []
    for i, l in enumerate(lines):
        m = re.match(r"([\w.$@?]+):", l)
        if m:
            pos[m.group(1)] = i
        kinds.append(bool(m))
    ranges = []
    for i, l in enumerate(lines):
        m = re.match(r"j\w+\s+([\w.$@?]+)", l)
        if m and m.group(1) in pos and pos[m.group(1)] <= i:
            ranges.append((pos[m.group(1)], i))
    inside = set()
    for a, b in ranges:
        inside.update(range(a, b + 1))
    ins = lambda r: sum(1 for k in r if not kinds[k])
    return ins(inside), sum(ins(range(a, b + 1)) for a, b in ranges)


def job(src):
    w = tempfile.mkdtemp(dir=WORK)
    try:
        r = subprocess.run([OURS, *FLAGS, LV, "-S", "--dump", w, *INC, "-o", w + "/o.s", str(src)],
                           capture_output=True, text=True)
        if r.returncode:
            return None
        st = glob.glob(w + "/*-switchconv.ll")
        if not st:
            return None
        ir = re.sub(r" tre-deferred| threeway", "", open(st[0]).read())
        out = []
        for extra in ([], ["-disable-machine-licm"]):
            d = pin.llc(["-O2", "-filetype=asm", "-o", "-", *extra], ir, of)
            if d.returncode:
                return None
            out.append((count(d.stdout), loops(d.stdout)))
        mine = open(w + "/o.s").read()
        return src.name, (count(mine), loops(mine)), out[0], out[1]
    finally:
        shutil.rmtree(w, ignore_errors=True)


srcs = [s for d in MODS for s in sorted((Q / d).glob("*.c"))] if MODE == "m16" else sorted(SRC.glob("*.c"))
with ThreadPoolExecutor(6) as ex:
    res = [r for r in ex.map(job, srcs) if r]
t = lambda i, k: sum((r[i][0] if k == 0 else r[i][1][k - 1]) for r in res)
print(f"{len(res)}/{len(srcs)} programs {LV} {MODE} same IR")
for k, name in ((0, "all"), (1, "in loops"), (2, "loop-weighted")):
    print(f"  {name:14} ours {t(1, k)}  pin {t(2, k)}  pin -disable-machine-licm {t(3, k)}")
for r in sorted(res, key=lambda r: r[3][1][0] - r[2][1][0], reverse=True)[:8]:
    print("  ", r)
