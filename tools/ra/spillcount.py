"""Six-GPR spill comparison: llrm-llc vs the pin llc on the same IR, both with
-frame-pointer=all and the C convention. Usage: spillcount.py OUTDIR IR..."""
import os, re, subprocess, sys

L = os.environ.get("LLRM_LLC", os.path.expanduser("~/scratch/w10-clean-target/release/llrm-llc"))
P = os.environ.get("PIN_LLC", os.path.expanduser("~/scratch/llvm-build/bin/llc"))
SLOT = re.compile(r"\[(?:e?bp)\s*-\s*\d+\]")
OPS = re.compile(r"(mov|movzx|movsx|add|sub|cmp|and|or|xor|imul|adc|sbb)\w*\s+(.*)")


def slots(path):
    st = ld = 0
    for line in open(path):
        m = OPS.match(line.strip())
        if not m or not SLOT.search(m.group(2)):
            continue
        dst = m.group(2).split(",")[0]
        if re.search(r"\[e?bp", dst) and m.group(1) == "mov":
            st += 1
        else:
            ld += 1
    return st + ld


out, files = sys.argv[1], sys.argv[2:]
os.makedirs(out, exist_ok=True)
tot = [0, 0]
for f in files:
    n = os.path.basename(f)[:-3]
    o, p = f"{out}/{n}.ours.s", f"{out}/{n}.pin.s"
    a = subprocess.run([L, "-m32", "-O2", "-march=i486", "--isel=dag", "-frame-pointer=all", "-mregcall-default", "-S", "-o", o, f], capture_output=True)
    b = subprocess.run([P, "-mtriple=i486-unknown-linux-gnu", "-mcpu=i486", "-mattr=-sse,-sse2,-mmx", "-O2", "-frame-pointer=all", "-x86-asm-syntax=intel", "-filetype=asm", f, "-o", p], capture_output=True)
    if a.returncode or b.returncode:
        continue
    x, y = slots(o), slots(p)
    tot[0] += x; tot[1] += y
    print(f"{n:16}{x:5}{y:5}{x - y:+5}")
print("total", *tot)
