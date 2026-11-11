#!/usr/bin/env python3
"""IR identity of two clangs over every row that uses the fork (clang_env LLRM_CLANG_M16): each .c of tests/run/c, tests/check/c, bench/*, -O1 -S -emit-llvm, ident lines stripped.
usage: sweep2.py OLD_CLANG NEW_CLANG OUTDIR"""
import sys, os, re, subprocess, tomllib, hashlib
from concurrent.futures import ThreadPoolExecutor
old, new, out = sys.argv[1:4]
R = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
rows = tomllib.load(open(f"{R}/crates/driver/llrm-cc/targets.toml", "rb"))
subst = {"{root}": R, "{ow}": os.path.expanduser("~/scratch/ow-src"), "{env:LLRM_BORLAND_INC}": "/tmp/vainc"}
def flags(row):
    f = []
    for w in row["clang"]:
        for k, v in subst.items(): w = w.replace(k, v)
        if "{" in w: return None
        f.append(w)
    return f
files = sorted(p for d in ("tests/run/c", "tests/check/c") for p in (os.path.join(R, d, x) for x in os.listdir(f"{R}/{d}")) if p.endswith(".c"))
for b in os.listdir(f"{R}/bench"):
    d = f"{R}/bench/{b}"
    if os.path.isdir(d): files += sorted(os.path.join(d, x) for x in os.listdir(d) if x.endswith(".c"))
def h(args):
    clang, fl, f = args
    r = subprocess.run([clang, "-S", "-emit-llvm", "-O1", *fl, f"-I{R}/tests/run/c", f"-I{os.path.dirname(f)}", "-o", "-", f], capture_output=True, text=True, cwd=os.path.dirname(f))
    body = "\n".join(l for l in r.stdout.splitlines() if not re.match(r"; ModuleID|source_filename|!llvm.ident|.*clang version", l))
    return (hashlib.md5(body.encode()).hexdigest()[:12] if r.returncode == 0 else "FAIL")
os.makedirs(out, exist_ok=True)
tot = {}
for name, row in rows.items():
    if row.get("clang_env") != "LLRM_CLANG_M16": continue
    fl = flags(row)
    if fl is None: print(name, "skipped: unresolved placeholder"); continue
    with ThreadPoolExecutor(6) as ex:
        a = list(ex.map(h, [(old, fl, f) for f in files])); b = list(ex.map(h, [(new, fl, f) for f in files]))
    ok = sum(1 for x in a if x != "FAIL"); both_fail = sum(1 for x, y in zip(a, b) if x == y == "FAIL")
    diff = [f for f, x, y in zip(files, a, b) if x != y]
    print(f"{name}: {len(files)} sources, {ok} compile on old, {both_fail} fail on both, {len(diff)} differ")
    open(f"{out}/{name}.diff", "w").write("\n".join(os.path.relpath(f, R) for f in diff))
