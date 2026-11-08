#!/usr/bin/env python3
"""Run from the repo root: python3 tools/wasm_c/run.py DIST OUTDIR [names]: tests/run/c on wasm32 in node. Per program and distinct (level + non-machine flags) variant:
llrm-cc -c, wasm-ld with runtime/wasm/libc.c, node run.mjs, compare report() lines with the .out (expected_output)."""
import os, sys, re, subprocess, tempfile, shutil
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
sys.path.insert(0, "tools"); import corpus_targets as ct, llrmbin
D, OUT = Path(sys.argv[1]), Path(sys.argv[2]); OUT.mkdir(parents=True, exist_ok=True)
names = sys.argv[3:]
C = Path("tests/run/c"); T = "wasm32"
PC = os.path.expanduser("~/scratch/llvm-build-clang-wasm/bin/clang")
env = dict(os.environ, **llrmbin.cc_environment(), LLRM_CC_TARGET=T, LLRM_CLANG_WASM=PC)
MACHINE = re.compile(r"^-(m32|march=|mabi=|mcmodel=|mlong-double|mregparm|mno-|msse|mfpmath|m16)")
def variants(src):
    for line in src.splitlines()[:6]:
        if line.startswith("// flags:"):
            vs = [v.split() for v in line.removeprefix("// flags:").split("|")]
            break
    else:
        vs = [["-O2"]]
    seen, res = set(), []
    for v in vs:
        key = tuple(w for w in v if not MACHINE.match(w))
        if any(w.startswith("-mabi=") or w.startswith("-mlong-double") or w == "-DREAL_MODE" for w in v): key = None  # ABI-changing: x86-only
        if key is None or key in seen: continue
        seen.add(key); res.append(list(key))
    return res
LIBC = OUT / "libc.o"
subprocess.run([PC, "--target=wasm32", "-ffreestanding", "-O2", "-fno-builtin", "-c", "runtime/wasm/libc.c", "-o", str(LIBC)], check=True)
subprocess.run([PC, "--target=wasm32", "-ffreestanding", "-O1", "-fno-builtin", "-c", str(Path(__file__).parent / "multi3.c"), "-o", str(OUT / "multi3.o")], check=True)
CRT = OUT / "crt.a"; CR = Path("runtime/compiler-rt"); objs = []
(OUT / "crt").mkdir(exist_ok=True)
for f in subprocess.run(["sh", str(CR / "tests/fp_sources.sh"), "f128"], capture_output=True, text=True, check=True).stdout.split() + ["lib/int/int128.c"]:
    o = OUT / "crt" / (f.replace("/", "_") + ".o")
    r = subprocess.run([PC, "--target=wasm32", "-ffreestanding", "-O2", "-fno-builtin", "-include", str(CR / "config/wasm/config.h"), "-I", str(CR / "include"), "-I", str(CR / "lib/fp/f128"), "-c", str(CR / f), "-o", str(o)], capture_output=True, text=True)
    if r.returncode == 0: objs.append(str(o))
    else: print("crt skip", f, r.stderr.strip().splitlines()[-1:] )
objs.append(str(OUT / "multi3.o")); CRT.unlink(missing_ok=True); subprocess.run(["ar", "rcs", str(CRT), *objs], check=True)
def one(job):
    name, flags = job; src = C / f"{name}.c"; lvl = next((w for w in flags if re.match(r"-O(\d|s|max)$", w)), "-O?")
    w = Path(tempfile.mkdtemp(prefix="wc-"))
    try:
        r = subprocess.run([str(D / "llrm-cc"), *flags, "-I", "bench", "-I", "runtime/shared/posix", "-I", "tests/run/c", "-c", str(src), "-o", f"{w}/p.o"], env=env, capture_output=True, text=True, timeout=300)
        if r.returncode: return (name, lvl, flags, "BUILD", (r.stderr.strip().splitlines() or [""])[-1][:150])
        r = subprocess.run(["wasm-ld", "--no-entry", "--allow-undefined", "--allow-multiple-definition", "--export-if-defined=main", "--export-if-defined=__main_void", "--export-if-defined=__wasm_call_ctors", "-z", "stack-size=1048576", f"{w}/p.o", str(CRT), str(LIBC), "-o", f"{w}/p.wasm"], capture_output=True, text=True)
        if r.returncode: return (name, lvl, flags, "LINK", r.stderr.strip()[:150])
        r = subprocess.run(["node", "--stack-size=2000", str(Path(__file__).parent / "run.mjs"), f"{w}/p.wasm"], capture_output=True, text=True, timeout=120)
        got = r.stdout
        if r.returncode == 3: return (name, lvl, flags, "NA", got.strip()[:150])
        want = ct.expected_output(src, T).read_text()
        if r.returncode == 0 and got == want: return (name, lvl, flags, "OK", "")
        if r.returncode == 0 and got.split() == want.split(): return (name, lvl, flags, "OK", "")
        gl, wl = got.splitlines(), want.splitlines()
        d = next((i for i in range(max(len(gl), len(wl))) if i >= len(gl) or i >= len(wl) or gl[i] != wl[i]), 0)
        return (name, lvl, flags, "MISMATCH", f"rc={r.returncode} line {d+1}: got {gl[d] if d < len(gl) else '<none>'} want {wl[d] if d < len(wl) else '<none>'}")
    except subprocess.TimeoutExpired:
        return (name, lvl, flags, "TIMEOUT", "")
    finally:
        shutil.rmtree(w, ignore_errors=True)
jobs = []; na = []
for p in sorted(C.glob("*.c")):
    if names and p.stem not in names: continue
    if not p.with_suffix(".out").exists(): continue
    s = p.read_text()
    if not ct.applies(T, s): na.append((p.stem, ct.reason(s))); continue
    jobs += [(p.stem, f) for f in variants(s)]
with ThreadPoolExecutor(int(os.environ.get("J", "4"))) as ex: rows = list(ex.map(one, jobs))
with open(OUT / "rows.tsv", "w") as f:
    for n, why in na: f.write(f"{n}\t-\t\tNA-target\t{why}\n")
    for n, l, fl, st, why in rows: f.write(f"{n}\t{l}\t{' '.join(fl)}\t{st}\t{why}\n")
from collections import Counter
print(Counter((l, st) for _, l, _, st, _ in rows)); print("NA-target", len(na))
