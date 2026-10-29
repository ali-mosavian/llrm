"""Premise test: ours against the pin's llc with and without one pass, on the same IR (m32 i486).

    tools/pairs/premise.py OURS_LLRM_CC LEVEL(-Os|-O2) PASSFLAG [N]

PASSFLAG: the llc switch that turns the pass off (--disable-peephole, --disable-machine-sink, -enable-misched=false).
IR: our pipeline's last MIR dump (`*-switchconv.ll`), compiled for m32 -mabi=sysv -march=i486. Measures instructions and
.text bytes per source, totals; `ours - pin_with` is the gap, `pin_without - pin_with` the pin's value of the pass.
"""
import re, subprocess, sys, tempfile, shutil, glob, os
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
sys.path.insert(0, str(Path(__file__).resolve().parent))
import pin

OURS, LV, FLAG = sys.argv[1], sys.argv[2], sys.argv[3]
os.environ["LLRM_CC_TARGET"] = "x86_32-wat"  # llrm-cc takes its target row from the environment
LIMIT = int(sys.argv[4]) if len(sys.argv) > 4 else 10**9
SRC = Path(os.environ.get("PREMISE_SRC") or Path.home() / "scratch/rows/main24d/b")  # the rows corpus: ROWS_WORK/main24d/b
of = pin.target("m32", optsize=(LV == "-Os"))
LLC = pin.PINNED_BIN / "llc"


def pin_asm(ir, extra, obj=False):
    cmd = [str(LLC), f"-mtriple={of.triple}", f"-mcpu={of.cpu}", f"-mattr={of.features}", "-O2", *extra,
           "-filetype=obj" if obj else "-filetype=asm", "-o", "-", "-"]
    d = subprocess.run(cmd, input=pin.retarget(ir, of).encode(), capture_output=True)
    return d.stdout if d.returncode == 0 else None


def text_bytes(obj):
    with tempfile.NamedTemporaryFile(suffix=".o") as f:
        f.write(obj); f.flush()
        out = subprocess.run(["size", "-A", f.name], capture_output=True, text=True).stdout
    m = re.search(r"^\.text\s+(\d+)", out, re.M)
    return int(m.group(1)) if m else 0


# Not instructions: our listing's directives (`public f`, `extern g`, `f endp`, `align 16`, `db`) began with a letter too and made ours 5.9% larger than it is.
NOT_INSTRUCTIONS = {"public", "extern", "end", "endp", "proc", "db", "dw", "dd", "align", "label"}


def count(asm):
    return sum(
        1 for l in asm.splitlines()
        if (m := re.match(r"\s+([a-z][a-z0-9]*)\b", l)) and not l.strip().startswith((".", "#")) and m.group(1) not in NOT_INSTRUCTIONS
    )


def job(src):
    w = tempfile.mkdtemp(dir=os.environ.get("PREMISE_TMP") or str(Path.home() / "scratch"))  # a dump on tmpfs is refused
    try:
        r = subprocess.run([OURS, "-m32", "-mabi=sysv", "-march=i486", LV, "-S", "--dump", w, "-o", w + "/o.s", str(src)], capture_output=True, text=True)
        if r.returncode: return None
        st = glob.glob(w + "/*-switchconv.ll")
        if not st: return None
        ir = re.sub(r" tre-deferred| threeway", "", open(st[0]).read())
        a, b = pin_asm(ir, []), pin_asm(ir, [FLAG])
        ao, bo = pin_asm(ir, [], True), pin_asm(ir, [FLAG], True)
        if None in (a, b, ao, bo): return None
        ours_ins = count(open(w + "/o.s").read())
        # our bytes: the object our compiler writes
        o = subprocess.run([OURS, "-m32", "-mabi=sysv", "-march=i486", LV, "-c", "-o", w + "/o.obj", str(src)], capture_output=True, text=True)
        return src.name, ours_ins, count(a.decode()), count(b.decode()), text_bytes(ao), text_bytes(bo)
    finally:
        shutil.rmtree(w, ignore_errors=True)


srcs = sorted(p for p in SRC.glob("*.c") if "." not in p.stem)[:LIMIT]
with ThreadPoolExecutor(8) as ex:
    res = [r for r in ex.map(job, srcs) if r]
tot = lambda i: sum(r[i] for r in res)
print(f"{len(res)} sources of {len(srcs)} at {LV}, m32 i486, same IR; pin switch {FLAG}")
print(f"  instructions: ours {tot(1)}  pin with {tot(2)}  pin without {tot(3)}   gap(ours-with) {tot(1)-tot(2)}  value(without-with) {tot(3)-tot(2)}")
print(f"  pin .text bytes: with {tot(4)}  without {tot(5)}  value {tot(5)-tot(4)}")
worst = sorted(res, key=lambda r: r[1] - r[2], reverse=True)[:5]
print("  largest instruction gaps (ours - pin with):", [(r[0], r[1] - r[2]) for r in worst])
