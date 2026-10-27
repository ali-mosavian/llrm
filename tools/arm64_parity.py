#!/usr/bin/env python3
"""arm64 Mac parity table: the pinned clang's optimizer+backend vs llrm-arm64-llc on one identical IR.

  tools/arm64_parity.py [-O O2,Os] [--runs N] [--md5] FILE.c|FILE.ll ...

A .c is made one unoptimized .ll (scalar: -fno-vectorize -fno-slp-vectorize, until vector MIR exists);
a .ll is used as given. Both sides start from it: `clang -O<n> -x ir` and llrm-arm64-llc.
Cells are __text bytes/instructions; ours/clang is the headline. Run time is the min of N runs
and means nothing near the process-start floor. A sibling .out/.status is the expected output.
A .c also goes through `llrm-cc` (target arm64-macos: the pin's front end, then the backend), a third column: the driver end to end.
Env: LLRM_PIN_ANY=1 (dry run with a non-pin clang), LLRM_PIN_BUILD (dir with bin/clang, bin/llc), LLRM_BIN (llrm binaries).
"""
import argparse, hashlib, os, re, subprocess, sys, tempfile, time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PIN = Path(os.environ.get("LLRM_PIN_BUILD", Path.home() / "work/other/llvm-pin/build"))
BIN = Path(os.environ.get("LLRM_BIN", REPO / "target/release"))
TARGET = ["-target", "arm64-apple-macos"]
FRONT = ["-ffp-contract=off", "-ffreestanding", "-fno-vectorize", "-fno-slp-vectorize"]  # "scalar clang": vector MIR is deferred


def sh(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


FRONT += ["-isysroot", sh(["xcrun", "--show-sdk-path"]).stdout.strip()]


def text_size(obj):
    """__TEXT,__text size in bytes, from `size -m` (not the whole-file size)."""
    out = sh(["size", "-m", str(obj)]).stdout
    m = re.search(r"Section \(__TEXT, __text\):\s+(\d+)", out)
    return int(m.group(1)) if m else None


def insn_count(obj):
    out = sh(["otool", "-tv", str(obj)]).stdout
    return sum(1 for l in out.splitlines() if re.match(r"^[0-9a-f]{16}\s", l))


SHIM = "#include <stdio.h>\nvoid report(long v){printf(\"%ld\\n\",v);}\n"  # the benches' output hook


def link(obj, exe):
    shim = exe.parent / "shim.o"
    if not shim.exists():
        (exe.parent / "shim.c").write_text(SHIM)
        sh(["clang", "-arch", "arm64", "-c", str(exe.parent / "shim.c"), "-o", str(shim)])
    r = sh(["clang", "-arch", "arm64", str(obj), str(shim), "-o", str(exe)])
    return r.returncode == 0, r.stderr.strip()


def run(exe, runs):
    best, out, st = None, None, None
    for _ in range(runs):
        t = time.perf_counter()
        r = subprocess.run([str(exe)], capture_output=True, timeout=120)
        dt = time.perf_counter() - t
        best = dt if best is None else min(best, dt)
        out, st = r.stdout, r.returncode
    return best, out, st


def build_variants(src, lvl, tmp, ver):
    """Returns {variant: obj path or error string}. One unoptimized .ll feeds both sides."""
    name = f"{src.stem}.{lvl}"
    v = {}
    if src.suffix == ".c":
        ll = tmp / f"{name}.ll"
        r = sh([str(PIN / "bin/clang"), f"-{lvl}", *TARGET, *FRONT, "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-lifetime-markers", "-S", "-emit-llvm", str(src), "-o", str(ll)])
        if r.returncode:
            return {"clang": "emit-llvm: " + r.stderr[:200], "ours": "emit-llvm"}
    else:
        ll = src
    o = tmp / f"{name}.clang.o"
    r = sh([str(PIN / "bin/clang"), f"-{lvl}", *TARGET, "-fno-vectorize", "-fno-slp-vectorize", "-Wno-override-module", "-x", "ir", "-c", str(ll), "-o", str(o)])
    v["clang"] = o if r.returncode == 0 else "clang: " + r.stderr[:200]
    o = tmp / f"{name}.ours.o"
    r = sh([str(BIN / "llrm-arm64-llc"), f"-{lvl}", "-o", str(o), str(ll)])
    v["ours"] = o if r.returncode == 0 else "ours: " + (r.stderr or r.stdout)[:200]
    if src.suffix == ".c" and (BIN / "llrm-cc").exists():
        o = tmp / f"{name}.cc.o"
        env = {**os.environ, "LLRM_CC_TARGET": "arm64-macos", "LLRM_PIN_CLANG": str(PIN / "bin/clang")}
        r = sh([str(BIN / "llrm-cc"), f"-{lvl}", *FRONT, "-c", str(src), "-o", str(o)], env=env)
        v["llrm-cc"] = o if r.returncode == 0 else "llrm-cc: " + (r.stderr or r.stdout)[:200]
    return v


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("-O", default="O2,Os")
    ap.add_argument("--runs", type=int, default=7)
    ap.add_argument("--md5", action="store_true", help="print md5 of every linked binary")
    a = ap.parse_args()
    if not (PIN / "bin/clang").exists():
        sys.exit(f"no pinned clang at {PIN}/bin/clang (set LLRM_PIN_BUILD)")
    ver = sh([str(PIN / "bin/clang"), "--version"]).stdout
    if "d1106deb71cc" not in ver and not os.environ.get("LLRM_PIN_ANY"):
        sys.exit("clang is not the pin d1106deb71cc")
    tmp = Path(tempfile.mkdtemp(prefix="parity."))
    (tmp / "empty.c").write_text("int main(void){return 0;}")
    sh([str(PIN / "bin/clang"), *TARGET, "-isysroot", FRONT[-1], str(tmp / "empty.c"), "-o", str(tmp / "empty")])
    print(f"process-start floor (empty main, min of {a.runs}): {run(tmp / 'empty', a.runs)[0]*1000:.1f} ms; run times near it are noise")
    cols = ["clang", "ours", "llrm-cc"]
    print(f"{'prog':<14}{'lvl':<4}" + "".join(f"{c:>16}" for c in cols) + f"{'ours/clang':>11}  run(ms) clang/ours  result")
    worse = 0
    for f in a.files:
        lv = [l for l in a.O.split(",") if l == "O2" or Path(f).suffix == ".c"]
        src = Path(f).resolve()
        for lvl in lv:
            v = build_variants(src, lvl, tmp, ver)
            cells, times, outs = [], [], {}
            for c in cols:
                o = v.get(c)
                if not isinstance(o, Path):
                    cells.append("-")
                    times.append("-")
                    if o:
                        print(f"  {src.stem} {lvl} {c}: {o}", file=sys.stderr)
                    continue
                b, n = text_size(o), insn_count(o)
                cells.append(f"{b}/{n}")
                exe = tmp / f"{src.stem}.{lvl}.{c}"
                ok, err = link(o, exe)
                if not ok:
                    times.append("link!")
                    print(f"  {src.stem} {lvl} {c}: link: {err[:200]}", file=sys.stderr)
                    continue
                t, out, st = run(exe, a.runs)
                times.append(f"{t*1000:.1f}")
                outs[c] = (out, st)
                if a.md5:
                    print(f"  md5(.o) {exe.name} {hashlib.md5(o.read_bytes()).hexdigest()}", file=sys.stderr)
            ratio = "-"
            base = v.get("clang")
            if isinstance(base, Path) and isinstance(v.get("ours"), Path):
                pb, ob = text_size(base), text_size(v["ours"])
                ratio = f"{ob/pb:.3f}"
                worse += ob > pb
            ref = outs.get("clang")
            exp = src.with_suffix(".out")
            if exp.exists() and src.suffix == ".ll":  # the recorded output beats any build's  # .ll with a recorded expectation
                st = src.with_suffix(".status")
                ref = (exp.read_bytes(), int(st.read_text()) if st.exists() else 0)
            res = "FAIL" if "ours" not in outs or "clang" not in outs else ("ok" if outs["ours"] == ref else "DIFF")
            if res == "ok" and "llrm-cc" in v:  # the driver's build must run as the others do
                res = "ok" if outs.get("llrm-cc") == ref else "CC-FAIL"
            print(f"{src.stem:<14}{lvl:<4}" + "".join(f"{c:>16}" for c in cells) + f"{ratio:>11}  " + "/".join(times) + f"  {res}")
    print(f"rows where ours is bigger than clang (bytes): {worse}; FAIL rows count against the corpus")


main()
