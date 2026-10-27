#!/usr/bin/env python3
"""arm64 Mac parity table: the pinned clang's optimizer+backend vs llrm-arm64-llc on one identical IR.

  tools/arm64_parity.py [-O O2,Os] [--runs N] [--md5] FILE.c|FILE.ll ...

A .c is made one unoptimized .ll (scalar: -fno-vectorize -fno-slp-vectorize, until vector MIR exists);
a .ll is used as given. Both sides start from it: `clang -O<n> -x ir` and llrm-arm64-llc.
Cells are __text bytes/instructions; ours/clang is the headline. Run time is the min of N runs
and means nothing near the process-start floor. A sibling .out/.status is the expected output.
A .c is also built by GCC (`gcc-13`, its own front end and optimizer, so the IR is not shared; LLRM_GCC overrides) as a fourth column.
A .c also goes through `llrm-cc` (target arm64-macos: the pin's front end, then the backend), a third column: the driver end to end.
Env: LLRM_PIN_ANY=1 (dry run with a non-pin clang), LLRM_PIN_BUILD (dir with bin/clang, bin/llc), LLRM_BIN (llrm binaries).
"""
import argparse, hashlib, math, os, re, shutil, subprocess, sys, tempfile, time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
PIN = Path(os.environ.get("LLRM_PIN_BUILD", Path.home() / "work/other/llvm-pin/build"))
BIN = Path(os.environ.get("LLRM_BIN", REPO / "target/release"))
TARGET = ["-target", "arm64-apple-macos"]
FRONT = ["-ffp-contract=off", "-ffreestanding", "-fno-vectorize", "-fno-slp-vectorize", "-Dmain=bench_main"]  # "scalar clang": vector MIR is deferred


def sh(cmd, **kw):
    return subprocess.run(cmd, capture_output=True, text=True, **kw)


FRONT += ["-isysroot", sh(["xcrun", "--show-sdk-path"]).stdout.strip()]
SDK = FRONT[-1]
FLOOR = 50e-9  # seconds per call below which a row is not counted


def text_size(obj):
    """__TEXT,__text size in bytes, from `size -m` (not the whole-file size)."""
    out = sh(["size", "-m", str(obj)]).stdout
    # Every code section: GCC puts `main` in __text_startup.
    sizes = re.findall(r"Section \(__TEXT, (__text[a-z_]*)\):\s+(\d+)", out)
    return sum(int(n) for _, n in sizes) if sizes else None


def insn_count(obj):
    size = text_size(obj)  # fixed-width instructions
    return None if size is None else size // 4


# The benches' hooks. A .c has its `main` renamed `bench_main` and is timed by the harness below: the first call
# prints its reports; then the batch doubles (1, 2, 4, ...) until it takes 500 ms, every call's reports checked against
# the first's, and the time per call is the last batch's. `keep` is opaque; `input_read` is 1 MiB of a fixed text
# (rewound each call) where the DOS benches read a file.
HARNESS = r"""
#include <stdio.h>
#include <time.h>
extern int bench_main(void);
static long first[64]; static int nfirst, at, pass, bad;
void report(long v) {
    if (pass == 0) { printf("%ld\n", v); if (nfirst < 64) first[nfirst++] = v; }
    else { if (at >= nfirst || first[at] != v) bad = 1; at++; }
}
int keep(int v) { return v; }
static long rd;
int input_read(char *b, int n) {
    static const char t[] = "the quick brown fox jumps over the lazy dog\nand then it was\n";
    int i = 0;
    while (i < n && rd < (1 << 20)) { b[i++] = t[rd % (sizeof t - 1)]; rd++; }
    return i;
}
static unsigned long long now(void) { return clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW); }
int main(void) {
    unsigned long long n = 1, dt;
    bench_main();
    pass = 1;
    for (;;) {
        unsigned long long t0 = now(), i;
        for (i = 0; i < n; ++i) { at = 0; rd = 0; bench_main(); }
        dt = now() - t0;
        if (bad) { puts("BAD"); return 1; }
        if (dt >= 500000000ULL) break;
        n *= 2;
    }
    printf("#ns %.3f %llu\n", (double)dt / n, n);
    return 0;
}
"""
PLAIN = "#include <stdio.h>\nvoid report(long v){printf(\"%ld\\n\",v);}\n"


def link(obj, exe, harness=True):
    shim = exe.parent / ("harness.o" if harness else "shim.o")
    if not shim.exists():
        (exe.parent / (shim.stem + ".c")).write_text(HARNESS if harness else PLAIN)
        sh(["clang", "-O2", "-arch", "arm64", "-c", str(exe.parent / (shim.stem + ".c")), "-o", str(shim)])
    r = sh(["clang", "-arch", "arm64", str(obj), str(shim), "-o", str(exe)])
    return r.returncode == 0, r.stderr.strip()


def run(exe, runs, harness=False):
    """(seconds per call, stdout, status): the harness's own figure, else the wall time of the whole process."""
    best, out, st = None, None, None
    for _ in range(runs):
        t = time.perf_counter()
        r = subprocess.run([str(exe)], capture_output=True, timeout=300)
        dt = time.perf_counter() - t
        out, st = r.stdout, r.returncode
        if harness:
            m = re.search(rb"#ns ([\d.]+) (\d+)\n", out)
            dt = float(m[1]) / 1e9 if m else None
            out = re.sub(rb"#ns [^\n]*\n", b"", out)
        if dt is not None:
            best = dt if best is None else min(best, dt)
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
    gcc = os.environ.get("LLRM_GCC", "gcc-13")
    if src.suffix == ".c" and shutil.which(gcc):
        o = tmp / f"{name}.gcc.o"
        r = sh([gcc, f"-{lvl}", "-mmacosx-version-min=15.0", "-fno-tree-vectorize", "-fno-tree-slp-vectorize", "-ffp-contract=off", "-ffreestanding", "-Dmain=bench_main", "-c", str(src), "-o", str(o)])
        v["gcc"] = o if r.returncode == 0 else "gcc: " + r.stderr[:200]
    return v


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="+")
    ap.add_argument("-O", default="O2,Os")
    ap.add_argument("--runs", type=int, default=1, help="repeats of each timed run (each takes 0.5-1 s)")
    ap.add_argument("--md5", action="store_true", help="print md5 of every linked binary")
    a = ap.parse_args()
    if not (PIN / "bin/clang").exists():
        sys.exit(f"no pinned clang at {PIN}/bin/clang (set LLRM_PIN_BUILD)")
    ver = sh([str(PIN / "bin/clang"), "--version"]).stdout
    if "d1106deb71cc" not in ver and not os.environ.get("LLRM_PIN_ANY"):
        sys.exit("clang is not the pin d1106deb71cc")
    tmp = Path(tempfile.mkdtemp(prefix="parity."))
    (tmp / "empty.c").write_text("int main(void){return 0;}")
    sh([str(PIN / "bin/clang"), *TARGET, "-isysroot", SDK, str(tmp / "empty.c"), "-o", str(tmp / "empty")])
    print("times: ns per call, the batch doubled until it takes 500 ms (a .ll is the whole process's wall time)")
    cols = ["clang", "ours", "llrm-cc", "gcc"]
    print(f"{'prog':<14}{'lvl':<4}" + "".join(f"{c:>16}" for c in cols) + f"{'size o/clang':>13}{'o/gcc':>7}  ns/call clang/ours/cc/gcc  time o/clang o/gcc  result")
    worse = 0
    tc, tg = [], []
    skipped = 0
    for f in a.files:
        lv = [l for l in a.O.split(",") if l == "O2" or Path(f).suffix == ".c"]
        src = Path(f).resolve()
        for lvl in lv:
            v = build_variants(src, lvl, tmp, ver)
            cells, times, outs, secs = [], [], {}, {}
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
                ok, err = link(o, exe, src.suffix == ".c")
                if not ok:
                    times.append("link!")
                    print(f"  {src.stem} {lvl} {c}: link: {err[:200]}", file=sys.stderr)
                    continue
                t, out, st = run(exe, a.runs, src.suffix == ".c")
                times.append("BAD" if t is None else f"{t*1e9:.0f}")
                secs[c] = t
                outs[c] = (out, st)
                if a.md5:
                    print(f"  md5(.o) {exe.name} {hashlib.md5(o.read_bytes()).hexdigest()}", file=sys.stderr)
            ratio = "-"
            base = v.get("clang")
            if isinstance(base, Path) and isinstance(v.get("ours"), Path):
                pb, ob = text_size(base), text_size(v["ours"])
                ratio = f"{ob/pb:.3f}"
                worse += ob > pb
            gccb = v.get("gcc")
            gr = f"{text_size(v['ours'])/text_size(gccb):.3f}" if isinstance(gccb, Path) and isinstance(v.get("ours"), Path) else "-"
            ref = outs.get("clang")
            exp = src.with_suffix(".out")
            if exp.exists() and src.suffix == ".ll":  # the recorded output beats any build's  # .ll with a recorded expectation
                st = src.with_suffix(".status")
                ref = (exp.read_bytes(), int(st.read_text()) if st.exists() else 0)
            res = "FAIL" if "ours" not in outs or "clang" not in outs else ("ok" if outs["ours"] == ref else "DIFF")
            if res == "ok" and "llrm-cc" in v:  # the driver's build must run as the others do
                res = "ok" if outs.get("llrm-cc") == ref else "CC-FAIL"
            both = lambda x: secs.get("ours") and secs.get(x)
            rc = f"{secs['ours']/secs['clang']:.2f}" if both("clang") else "-"
            rg = f"{secs['ours']/secs['gcc']:.2f}" if both("gcc") else "-"
            # A call under FLOOR is a constant some compiler folded: no ratio of it means anything.
            counted = res == "ok" and src.suffix == ".c" and min(secs.values(), default=0) >= FLOOR
            if counted and both("clang"):
                tc.append(secs["ours"] / secs["clang"])
            if counted and both("gcc"):
                tg.append(secs["ours"] / secs["gcc"])
            skipped += res == "ok" and src.suffix == ".c" and not counted
            print(f"{src.stem:<14}{lvl:<4}" + "".join(f"{c:>16}" for c in cells) + f"{ratio:>13}{gr:>7}  " + "/".join(times) + f"  {rc:>6}{rg:>6}  {res}")
    geo = lambda x: math.exp(sum(map(math.log, x)) / len(x)) if x else float("nan")
    print(f"time, geometric mean of ours/clang: {geo(tc):.3f} ({len(tc)} rows); ours/gcc: {geo(tg):.3f} ({len(tg)} rows); {skipped} rows left out: some build ran in under {FLOOR * 1e9:.0f} ns (folded)")
    print(f"rows where ours is bigger than clang (bytes): {worse}; FAIL rows count against the corpus")


main()
