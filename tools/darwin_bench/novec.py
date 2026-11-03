"""novec.py LEVEL prog... : pin and gcc-13 with and without vectorization, next to llrm-cc (LLRM_CC).
Per cell: kernel bytes, kernel vector insns, us/run (median), output ok. A cell is "vector-deferred" when the
vectorizing best beats llrm-cc by >10% in us or bytes and the non-vectorizing best does not.
Header: compiled-cell counts and md5 of every compiler binary."""
import sys, os, subprocess, re, statistics, hashlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from codebytes import kernel
from cmp import regressions_vs_best
import host
H = os.path.dirname(os.path.abspath(__file__)); WORK = os.environ.get("BENCH_WORK", "/tmp/darwin-bench"); os.makedirs(WORK, exist_ok=True); REPO = os.environ.get("BENCH_REPO", os.getcwd())
PIN = os.environ["LLRM_PIN_CLANG"]; GCC = "/opt/homebrew/bin/gcc-13"; LLRM = os.environ["LLRM_CC"]
SDK = subprocess.check_output(["xcrun", "--show-sdk-path"], text=True).strip()
CC = {"pin": ([PIN, "-isysroot", SDK], []),
      "pin-nv": ([PIN, "-isysroot", SDK], ["-fno-vectorize", "-fno-slp-vectorize"]),
      "gcc13": ([GCC, "-isysroot", SDK, "-mmacosx-version-min=14.0"], []),
      "gcc13-nv": ([GCC, "-isysroot", SDK, "-mmacosx-version-min=14.0"], ["-fno-tree-vectorize", "-fno-tree-slp-vectorize"]),
      "llrm-cc": ([LLRM, "--target=arm64-apple-macosx", "-isysroot", SDK], [])}
level, progs = sys.argv[1], sys.argv[2:]
REPS = int(os.environ.get("REPS", "11")); BUDGET_US = float(os.environ.get("BUDGET_US", "150000"))
def sh(c, **k):
    try: return subprocess.run(c, capture_output=True, text=True, timeout=120, **k)
    except subprocess.TimeoutExpired: return subprocess.CompletedProcess(c, 124, "", "timeout")
md5 = lambda p: hashlib.md5(open(p, "rb").read()).hexdigest()
HOSTOBJ = host.objects(PIN, SDK, WORK + "/host", REPO)
drv = WORK + "/driver.o"; sh(CC["pin"][0] + ["-O2", "-c", H + "/driver.c", "-o", drv])
shim = WORK + "/input_shim.o"; sh(CC["pin"][0] + ["-O2", "-c", H + "/input_shim.c", "-o", shim])
VEC = re.compile(r"^[0-9a-f]{16}\t(?!movi)\w+\.(?:2d|4s|2s|8h|4h|16b|8b)\b")  # SIMD arithmetic; q-register memset stores are not vectorization
def kernel_stats(o):
    cur = None; n = 0; vec = 0
    for l in sh(["otool", "-tv", o]).stdout.splitlines():
        m = re.match(r"^(\S+):$", l)
        if m: cur = m[1]; continue
        if cur in ("_orig_main", "_report") or not re.match(r"^[0-9a-f]{16}\t", l): continue
        n += 1; vec += bool(VEC.search(l))
    return n, vec
rows = {}; cells = fails = 0
for prog in progs:
    want = None if prog == "grep" else [int(x) % 2**32 for x in open(f"{REPO}/bench/{prog}/{prog}.out").read().split()]
    src = open(f"{REPO}/bench/{prog}/{prog}.c").read()
    src = re.sub(r"^((?:unsigned )?(?:long|int|short)\s+bench_)", r"__attribute__((noinline)) \1", src, flags=re.M)
    src = "static volatile int bench_zero;\n" + re.sub(r"(\bbench_\w+\()(\d+)\)", r"\1bench_zero + \2)", src)
    for name, (cc, extra) in CC.items():
        cells += 1; d = f"{WORK}/novec/{prog}.{level}.{name}"; os.makedirs(d, exist_ok=True)
        open(f"{d}/src.c", "w").write(src)
        env = {**os.environ, "LLRM_PIN_CLANG": PIN}
        r = sh(cc + [f"-{level}", "-I", f"{REPO}/bench", *host.include_flags(REPO, prog), *host.FLAT, "-Dmain=orig_main", "-c", f"{d}/src.c", "-o", f"{d}/k.o"] + extra, env=env)
        if r.returncode: fails += 1; rows[(prog, name)] = "compile failed"; continue
        if sh([PIN, "-isysroot", SDK, f"{d}/k.o", drv, *HOSTOBJ] + ([shim] if prog == "grep" else []) + ["-o", f"{d}/t"]).returncode: fails += 1; rows[(prog, name)] = "link failed"; continue
        host.write_data(REPO, prog, d)
        got = [int(x) % 2**32 for x in sh([f"{d}/t", "1", "1", "once"], cwd=d).stdout.split()]
        o1 = sh([f"{d}/t", "1", "1"], cwd=d).stdout.split()
        if not o1: fails += 1; rows[(prog, name)] = "run failed"; continue
        k = max(1, int(BUDGET_US / max(float(o1[0]), 0.1)))
        t = sorted(host.timings(sh, f"{d}/t", d, k, REPS))
        n, vec = kernel_stats(f"{d}/k.o")
        rows[(prog, name)] = (want is None or got == want, kernel(f"{d}/k.o")[0], n, vec, statistics.median(t))
print(f"{level}: cells compiled {cells - fails}/{cells}  load {os.getloadavg()}")
for n, p in (("pin clang", PIN), ("gcc-13", GCC), ("llrm-cc", LLRM)): print(f"  {n} md5 {md5(p)}")
print(f"{'prog':11}" + "".join(f"{n:>24}" for n in CC) + "  class")
print(" " * 11 + "".join(f"{'ok B insn vec us':>24}" for _ in CC))
for prog in progs:
    line = f"{prog:11}"; v = {}
    for name in CC:
        r = rows.get((prog, name))
        if isinstance(r, tuple): ok, b, n, vec, us = r; v[name] = r; line += f"{'Y' if ok else 'N'} {b:5} {n:4} {vec:3} {us:8.2f}".rjust(24)
        else: line += str(r).rjust(24)
    cls = ""
    if "llrm-cc" in v and all(k in v for k in ("pin", "gcc13", "pin-nv", "gcc13-nv")):
        l = v["llrm-cc"]
        cell = lambda r: (r[1], r[2], r[4])
        bad = lambda names: regressions_vs_best(cell(l), [cell(v[x]) for x in names], (1.10,) * 3)
        vec_bad = bad(("pin", "gcc13")); nv_bad = bad(("pin-nv", "gcc13-nv"))
        cls = "vector-deferred" if vec_bad and not nv_bad else ("worse (also without vectors)" if nv_bad else "")
        if not all(v[k][0] for k in v): cls += " WRONG-OUTPUT"
    print(line + "  " + cls)
