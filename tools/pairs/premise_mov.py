"""Per-function mov gap, ours against the pin's llc with and without --disable-peephole (same MIR, m32 i486): premise_mov.py OURS_LLRM_CC LEVEL"""
import os, re, sys, subprocess, tempfile, shutil, glob, collections
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
sys.path.insert(0, str(Path(__file__).resolve().parent))
import pin
OURS, LV = sys.argv[1], sys.argv[2]
os.environ["LLRM_CC_TARGET"] = "x86_32-wat"  # llrm-cc takes its target row from the environment
SRC = Path(os.environ.get("PREMISE_SRC") or Path.home() / "scratch/rows/main24d/b")
of = pin.target("m32", optsize=(LV == "-Os"))
LLC = pin.PINNED_BIN / "llc"
SUFFIX = {"mov","add","sub","and","or","xor","cmp","test","lea","inc","dec","neg","not","shl","shr","sar","imul","push","pop","adc","sbb","movz","movs","jmp"}
def norm(m):
    m = m.lower()
    if m.startswith(("movz","movs")) and m not in ("movsd",) : return "movzx" if m.startswith("movz") else "movsx"
    if m in ("cltd","cdq"): return "cdq"
    if m.startswith("f") and m[:-1] in ("fld","fst","fstp","fadd","fsub","fmul","fdiv","fsubr","fdivr","fild","fistp","fist","fcom","fcomp","fiadd","fimul") and m[-1] in "slt": return m[:-1]
    if m in ("fldt","fstpt"): return m[:-1]
    if m in ("retl","ret"): return "ret"
    if m in ("calll","call"): return "call"
    for b in sorted(SUFFIX, key=len, reverse=True):
        if m == b + "l" or m == b + "w" or m == b + "b": return b
    return m
def per_function(asm):
    """name -> mov count; a function starts at a label that is not local (.L, ..)."""
    out, cur = collections.Counter(), None
    for l in asm.splitlines():
        m = re.match(r"([A-Za-z_$][A-Za-z0-9_.$@]*):", l) or re.match(r"([A-Za-z_$][A-Za-z0-9_.$@]*)\s+proc\b", l)
        if m and not l.startswith(".") and not re.match(r"L\d+_", l):
            cur = m.group(1).lstrip("_"); out[cur] += 0; continue
        m = re.match(r"\s+([a-z][a-z0-9]*)\b", l)
        if cur and m and norm(m.group(1)) == "mov": out[cur] += 1
    return out
def pin_asm(ir, extra):
    d = subprocess.run([str(LLC), f"-mtriple={of.triple}", f"-mcpu={of.cpu}", f"-mattr={of.features}", "-O2", *extra, "-filetype=asm", "-o", "-", "-"], input=pin.retarget(ir, of), capture_output=True, text=True)
    return d.stdout if d.returncode == 0 else None
def job(src):
    w = tempfile.mkdtemp(dir=os.environ.get("PREMISE_TMP") or str(Path.home() / "scratch"))
    try:
        r = subprocess.run([OURS, "-m32", "-mabi=sysv", "-march=i486", LV, "-S", "--dump", w, "-o", w + "/o.s", str(src)], capture_output=True, text=True)
        if r.returncode: return None
        st = glob.glob(w + "/*-switchconv.ll")
        if not st: return None
        ir = re.sub(r" tre-deferred| threeway", "", open(st[0]).read())
        a, b = pin_asm(ir, []), pin_asm(ir, ["--disable-peephole"])
        if a is None or b is None: return None
        return src.name, per_function(open(w + "/o.s").read()), per_function(a), per_function(b)
    finally: shutil.rmtree(w, ignore_errors=True)
srcs = sorted(p for p in SRC.glob("*.c") if "." not in p.stem)
with ThreadPoolExecutor(8) as ex: res = [r for r in ex.map(job, srcs) if r]
rows = []
for name, ours, with_, without in res:
    for fn in with_:
        if fn in ours: rows.append((ours[fn] - with_[fn], without.get(fn, 0) - with_[fn], name, fn, ours[fn], with_[fn], without.get(fn, 0)))
rows.sort(reverse=True)
print(len(res), "programs", LV, "; per function mov: ours, pin with, pin without; gap = ours - with")
print("functions matched", len(rows), "; gap>0:", sum(1 for r in rows if r[0] > 0), "gap<0:", sum(1 for r in rows if r[0] < 0), "sum gap", sum(r[0] for r in rows), "sum value", sum(r[1] for r in rows))
for gap, value, name, fn, o, w_, wo in rows[:25]:
    print(f"  {name}:{fn} ours {o} with {w_} without {wo} gap {gap:+d} value {value:+d}")
