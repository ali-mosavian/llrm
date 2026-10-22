"""Per-mnemonic gap, ours against the pin's llc with and without --disable-peephole (same MIR, m32 i486): classify.py OURS_LLRM_C LEVEL"""
import os, re, sys, subprocess, tempfile, shutil, glob, collections
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
sys.path.insert(0, str(Path(__file__).resolve().parent))
import pin
OURS, LV = sys.argv[1], sys.argv[2]
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
def hist(asm):
    c = collections.Counter()
    for l in asm.splitlines():
        m = re.match(r"\s+([a-z][a-z0-9]*)\b", l)
        if m and not l.strip().startswith((".", "#")) and m.group(1) not in ("db","dw","dd","align","public","extern","end","endp","proc"): c[norm(m.group(1))] += 1
    return c
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
        return src.name, hist(open(w + "/o.s").read()), hist(a), hist(b)
    finally: shutil.rmtree(w, ignore_errors=True)
srcs = sorted(p for p in SRC.glob("*.c") if "." not in p.stem)
with ThreadPoolExecutor(8) as ex: res = [r for r in ex.map(job, srcs) if r]
tot = [collections.Counter(), collections.Counter(), collections.Counter()]
for r in res:
    for i in range(3): tot[i].update(r[i + 1])
print(len(res), "programs", LV, "; mnemonic: ours, pin with peephole, pin without; gap = ours - with; value = without - with")
rows = sorted(set(tot[0]) | set(tot[1]) | set(tot[2]), key=lambda m: -abs(tot[0][m] - tot[1][m]))
for m in rows[:22]:
    print(f"  {m:8} ours {tot[0][m]:5} with {tot[1][m]:5} without {tot[2][m]:5}   gap {tot[0][m]-tot[1][m]:+5}   value {tot[2][m]-tot[1][m]:+5}")
print("  total ours", sum(tot[0].values()), "with", sum(tot[1].values()), "without", sum(tot[2].values()))
