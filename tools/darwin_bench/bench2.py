"""bench2.py LEVEL prog... : llrm-cc from two builds (LLRM_CC_HEAD, LLRM_CC_BASE) on the bench programs.
Per program: kernel bytes (all code sections minus orig_main/report), static kernel insns, us/run, output ok.
Run from a checkout holding bench/. Uses the pilot's source patching (noinline kernel, runtime args)."""
import sys, os, subprocess, re, statistics
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from codebytes import kernel
from cmp import regressions
import host
H = os.path.dirname(os.path.abspath(__file__)); WORK = os.environ.get("BENCH_WORK", "/tmp/darwin-bench"); os.makedirs(WORK, exist_ok=True); REPO = os.environ.get("BENCH_REPO", os.getcwd())
SDK = subprocess.check_output(["xcrun", "--show-sdk-path"], text=True).strip()
BUILDS = {"head": os.environ["LLRM_CC_HEAD"], "base": os.environ["LLRM_CC_BASE"]}
level, progs = sys.argv[1], sys.argv[2:]
REPS = int(os.environ.get("REPS", "11")); BUDGET_US = float(os.environ.get("BUDGET_US", "150000"))
import resource
def _cap():  # one runaway compile must not take the box: 16 GB address space (macOS may not enforce it)
    try: resource.setrlimit(resource.RLIMIT_AS, (16 << 30, 16 << 30))
    except (ValueError, OSError): pass
def sh(c, **k):
    try: return subprocess.run(c, capture_output=True, text=True, timeout=120, preexec_fn=_cap, **k)
    except subprocess.TimeoutExpired: return subprocess.CompletedProcess(c, 124, "", "timeout")
PIN = os.environ["LLRM_PIN_CLANG"]
shim = WORK + "/input_shim.o"; sh([PIN, "-isysroot", SDK, "-O2", "-c", H + "/input_shim.c", "-o", shim])
drv = WORK + "/driver.o"; sh([PIN, "-isysroot", SDK, "-O2", "-c", H + "/driver.c", "-o", drv])
HOSTOBJ = host.objects(PIN, SDK, WORK + "/host", REPO)
rows = []
for prog in progs:
    want = None if prog == "grep" else [int(x) % 2**32 for x in open(f"{REPO}/bench/{prog}/{prog}.out").read().split()]
    outs = {}
    src = open(f"{REPO}/bench/{prog}/{prog}.c").read()
    src = re.sub(r"^((?:unsigned )?(?:long|int|short)\s+bench_)", r"__attribute__((noinline)) \1", src, flags=re.M)
    src = "static volatile int bench_zero;\n" + re.sub(r"(\bbench_\w+\()(\d+)\)", r"\1bench_zero + \2)", src)
    for name, cc in BUILDS.items():
        d = f"{WORK}/cmp/{prog}.{level}.{name}"; os.makedirs(d, exist_ok=True)
        open(f"{d}/src.c", "w").write(src)
        env = {**os.environ, "LLRM_PIN_CLANG": PIN}
        r = sh([cc, "--target=arm64-apple-macosx", "-isysroot", SDK, f"-{level}", "-I", f"{REPO}/bench", *host.include_flags(REPO, prog), *host.FLAT,
                "-Dmain=orig_main", "-c", f"{d}/src.c", "-o", f"{d}/k.o"], env=env)
        if r.returncode: rows.append((prog, name, "compile failed " + r.stderr[:100].replace("\n", " "))); continue
        r = sh([PIN, "-isysroot", SDK, f"{d}/k.o", drv, *HOSTOBJ] + ([shim] if prog == "grep" else []) + ["-o", f"{d}/t"])
        if r.returncode: rows.append((prog, name, "link failed")); continue
        host.write_data(REPO, prog, d)
        got = [int(x) % 2**32 for x in sh([f"{d}/t", "1", "1", "once"], cwd=d).stdout.split()]
        outs[name] = got
        ok = True if want is None else got == want
        kb, insns = kernel(f"{d}/k.o")
        o1 = sh([f"{d}/t", "1", "1"], cwd=d).stdout.split()
        if not o1: rows.append((prog, name, "run failed")); continue
        k = max(1, int(BUDGET_US / max(float(o1[0]), 0.1)))
        t = sorted(host.timings(sh, f"{d}/t", d, k, REPS))
        rows.append((prog, name, ok, kb, insns, statistics.median(t)))
    if want is None and len(outs) == 2 and outs["head"] != outs["base"]:
        rows = [(r[0], r[1], False) + r[3:] if r[0] == prog else r for r in rows]
print("load at end:", os.getloadavg())
by = {}
for r in rows:
    by.setdefault(r[0], {})[r[1]] = r[2:]
print(f"{level}: prog  kernelB(base head) insns(base head) us(base head)  flags")
worse = []; same = []
for p, d in by.items():
    if "head" not in d or "base" not in d or isinstance(d["head"][0], str) or isinstance(d["base"][0], str):
        print(p, d); continue
    (okh, kbh, ih, th), (okb, kbb, ib, tb) = d["head"], d["base"]
    flag = ("" if okh else " WRONG-OUTPUT-HEAD") + ("" if okb else " WRONG-OUTPUT-BASE")
    line = f"{p:13}{kbb:6}{kbh:6} |{ib:5}{ih:5} |{tb:9.2f}{th:9.2f}{flag}"
    if regressions((kbh, ih, th), (kbb, ib, tb)) or flag: worse.append(line)
    else: same.append(line)
print("WORSE (head > base) first:"); print("\n".join(worse) or "none")
print("NOT WORSE:"); print("\n".join(same))
