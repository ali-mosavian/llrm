"""Premise test of a pin pass: ours against the pin's llc with and without a switch that turns the pass off, on our IR.

    pinswitch.py LLRM_C LEVEL(-O2|-Os) SRC_DIR m32|m16 SWITCH   (m16 compiles the QCport modules under ~/scratch/qcport/src)

Prints instructions, in-loop instructions and jumps (jmp and jcc) of ours, the pin, and the pin with SWITCH.
"""
import glob, re, shutil, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pin

OURS, LV, SRC = sys.argv[1], sys.argv[2], Path(sys.argv[3])
MODE = sys.argv[4]
SWITCH = sys.argv[5]
of = pin.target(MODE, optsize=(LV == "-Os"))
Q = Path.home() / "scratch/qcport/src"
MODS = ("host", "render", "model", "game", "sound", "ui")
INC = [x for d in (*MODS, "qgl") for x in ("-I", str(Q / d))] + ["-I", str(Path.home() / "scratch/qctc/inc")] if MODE == "m16" else []
FLAGS = ["-m16"] if MODE == "m16" else ["-m32", "-mabi=sysv", "-march=i486"]
WORK = Path.home() / "scratch/w3-mlicm/tmp"
WORK.mkdir(exist_ok=True)


def jumps(asm):
    return sum(1 for l in asm.splitlines() if re.match(r"\s+j\w+\s", l))


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
        for extra in ([], [SWITCH]):
            d = pin.llc(["-O2", "-filetype=asm", "-o", "-", *extra], ir, of)
            if d.returncode:
                return None
            out.append((count(d.stdout), loops(d.stdout), jumps(d.stdout)))
        mine = open(w + "/o.s").read()
        return src.name, (count(mine), loops(mine), jumps(mine)), out[0], out[1]
    finally:
        shutil.rmtree(w, ignore_errors=True)


srcs = [s for d in MODS for s in sorted((Q / d).glob("*.c"))] if MODE == "m16" else sorted(SRC.glob("*.c"))
with ThreadPoolExecutor(6) as ex:
    res = [r for r in ex.map(job, srcs) if r]
def t(i, k):
    pick = {"all": lambda r: r[i][0], "in loops": lambda r: r[i][1][0], "jumps": lambda r: r[i][2]}
    return sum(pick[k](r) for r in res)


print(f"{len(res)}/{len(srcs)} programs {LV} {MODE} same IR, switch {SWITCH}")
for k in ("all", "in loops", "jumps"):
    print(f"  {k:10} ours {t(1, k)}  pin {t(2, k)}  pin with {SWITCH} {t(3, k)}")
