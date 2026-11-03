"""sameir.py LEVEL prog... : pin's optimized IR (-fno-vectorize: llrm-cc has no vectorizer and llc refuses vector IR; compared with pin-nv) through llrm-arm64-llc, between pin and llrm-cc.
llc(pinIR) vs pin  = backend gap; llrm-cc vs llc(pinIR) = mid-end/driver gap. Needs novec.py's cells for LEVEL.
Per prog: kernel B / static insns for pin, llc-dag(pinIR), llc-hand(pinIR), llrm-cc. LLC=llrm-arm64-llc path."""
import sys, os, subprocess, re, statistics
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from codebytes import kernel
H = os.path.dirname(os.path.abspath(__file__)); WORK = os.environ.get("BENCH_WORK", "/tmp/darwin-bench"); REPO = os.environ.get("BENCH_REPO", os.getcwd())
PIN = os.environ["LLRM_PIN_CLANG"]; LLC = os.environ["LLC"]; SDK = subprocess.check_output(["xcrun", "--show-sdk-path"], text=True).strip()
level, progs = sys.argv[1], sys.argv[2:]
def sh(c, **k):
    try: return subprocess.run(c, capture_output=True, text=True, timeout=120, **k)
    except subprocess.TimeoutExpired: return subprocess.CompletedProcess(c, 124, "", "timeout")
drv = WORK + "/driver.o"; shim = WORK + "/input_shim.o"
def measure(prog, o):
    t = o[:-2] + ".t"
    if sh([PIN, "-isysroot", SDK, o, drv] + ([shim] if prog == "grep" else []) + ["-o", t]).returncode: return None
    o1 = sh([t, "1", "1"]).stdout.split()
    if not o1: return None
    k = max(1, int(150000 / max(float(o1[0]), 0.1)))
    return statistics.median(float(x) for x in sh([t, str(k), "11"]).stdout.split())
print(f"{level}: prog | pin B/insn | llc-dag(pinIR) | llc-hand(pinIR) | llrm-cc")
for prog in progs:
    d = f"{WORK}/novec/{prog}.{level}.pin-nv"; ll = f"{d}/pin.ll"
    r = sh([PIN, "-isysroot", SDK, f"-{level}", "-I", f"{REPO}/bench", "-Dmain=orig_main", "-fno-vectorize", "-fno-slp-vectorize", "-S", "-emit-llvm", f"{d}/src.c", "-o", ll])
    cols = []
    for lab, o in (("pin", f"{d}/k.o"), ("dag", f"{d}/dag.o"), ("hand", f"{d}/hand.o"), ("cc", f"{WORK}/novec/{prog}.{level}.llrm-cc/k.o")):
        if lab in ("dag", "hand"):
            mode = lab; llo = {"dag": "2", "hand": "2"}[lab]
            if sh([LLC, "-O" + ("2" if level == "Os" else level[1]), f"--isel={mode}", "-o", o, ll]).returncode: cols.append(f"{lab} llc failed"); continue
        cols.append(f"{kernel(o)[0]}/{kernel(o)[1]}")
    print(f"{prog:13}" + " | ".join(f"{c:>16}" for c in cols), flush=True)
