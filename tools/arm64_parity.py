#!/usr/bin/env python3
"""arm64 Mac parity table: pinned clang vs llrm-arm64-llc, per program and level.

  tools/arm64_parity.py [-O O2,Os] [--runs N] [--md5] FILE.c|FILE.ll ...

Columns per level: clang (whole pipeline), pin-llc (clang IR through the pin's llc),
ours (the same IR through llrm-arm64-llc). Cells are text bytes/instructions; run
time is min of N runs. pin-llc vs ours is the backend gap; clang vs pin-llc is
the noise of the instrument (should be 0). .ll inputs skip the clang column.
Env: LLRM_PIN_ANY=1 (dry run with a non-pin clang), LLRM_PIN_BUILD (dir with bin/clang, bin/llc), LLRM_BIN (llrm binaries).
"""
import argparse, hashlib, os, re, subprocess, sys, tempfile, time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PIN = Path(os.environ.get("LLRM_PIN_BUILD", Path.home() / "work/other/llvm-project/build"))
BIN = Path(os.environ.get("LLRM_BIN", REPO / "target/release"))
TARGET = ["-target", "arm64-apple-macos"]
FRONT = ["-ffp-contract=off", "-ffreestanding"]


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


def link(obj, exe):
    r = sh(["clang", "-arch", "arm64", str(obj), "-o", str(exe)])
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
    """Returns {variant: obj path or error string}."""
    name = f"{src.stem}.{lvl}"
    v = {}
    if src.suffix == ".c":
        o = tmp / f"{name}.clang.o"
        r = sh([str(PIN / "bin/clang"), f"-{lvl}", *TARGET, *FRONT, "-c", str(src), "-o", str(o)])
        v["clang"] = o if r.returncode == 0 else "clang: " + r.stderr[:200]
        ll = tmp / f"{name}.ll"
        r = sh([str(PIN / "bin/clang"), f"-{lvl}", *TARGET, *FRONT, "-S", "-emit-llvm", str(src), "-o", str(ll)])
        if r.returncode:
            return v | {"pin-llc": "emit-llvm: " + r.stderr[:200], "ours": "emit-llvm"}
    else:
        ll = src
    o = tmp / f"{name}.pin.o"
    r = sh([str(PIN / "bin/llc"), "-O2", "-mtriple=arm64-apple-macos", "-filetype=obj", str(ll), "-o", str(o)])
    v["pin-llc"] = o if r.returncode == 0 else "llc: " + r.stderr[:200]
    o = tmp / f"{name}.ours.o"
    r = sh([str(BIN / "llrm-arm64-llc"), "-o", str(o), str(ll)])
    v["ours"] = o if r.returncode == 0 else "ours: " + (r.stderr or r.stdout)[:200]
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
    cols = ["clang", "pin-llc", "ours"]
    print(f"{'prog':<14}{'lvl':<4}" + "".join(f"{c:>16}" for c in cols) + f"{'ours/pin':>10}  run(ms) clang/pin/ours  result")
    worse = 0
    for f in a.files:
        src = Path(f).resolve()
        for lvl in a.O.split(","):
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
            if v.get("pin-llc") and isinstance(v["pin-llc"], Path) and isinstance(v.get("ours"), Path):
                pb, ob = text_size(v["pin-llc"]), text_size(v["ours"])
                ratio = f"{ob/pb:.3f}"
                worse += ob > pb
            ref = outs.get("clang") or outs.get("pin-llc")
            exp = src.with_suffix(".out")
            if ref is None and exp.exists():  # .ll with a recorded expectation
                st = src.with_suffix(".status")
                ref = (exp.read_bytes(), int(st.read_text()) if st.exists() else 0)
            res = "-" if "ours" not in outs else ("ok" if outs["ours"] == ref else "DIFF")
            print(f"{src.stem:<14}{lvl:<4}" + "".join(f"{c:>16}" for c in cells) + f"{ratio:>10}  " + "/".join(times) + f"  {res}")
    print(f"programs worse than pin-llc (bytes): {worse}")


main()
