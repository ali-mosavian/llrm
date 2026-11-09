#!/usr/bin/env python3
"""Refit `function_scale` of the m68k size table (crates/target/llrm-m68k/src/opcosts.rs).

m68k_size_fit.py [--llc PATH] [--nm PATH] [--dir DIR] [--levels O2,Os]

The estimate (`size_costs`, summed per function over the IR the backend gets) leaves out what the lowering adds: spills, copies, branch
relaxation. `function_scale` is that residue: the MEDIAN per-function ratio of the object's symbol size to the table's own estimate
(the many small functions the byte decisions are about set the median; the aggregate is lower because big functions carry the
residue less than proportionally).

Per CPU, over the `.ll` files of DIR (default tests/data_size: the last IR stage of 22 tests/run/c programs), or with --corpus over
every tests/run/c program the CPU applies to (clang's IR, as `llrm-cc -S -emit-llvm` hands the driver; --cc is that llrm-cc): each is compiled with
`llrm-m68k-llc -c --size-estimate=` at each level, the object's function sizes read with `nm -S`, and a ratio taken per function.
Train and hold-out are split by crc32 of `file:function` (one in five is held out). Prints, per CPU: the median on the train split
(the scale to set, in percent), the hold-out median absolute error at that scale and at 1.0, and the aggregate estimate/object.

Run it when `tests/size_calibration.rs` goes red: the backend got smaller (or larger) and the residue moved.
Environment: GCC_M68K (unpacked m68k binutils) for nm when --nm is not given.
"""
import argparse
import os
import statistics
import subprocess
import sys
import tempfile
import zlib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CPUS = ["68000", "68020"]


def function_sizes(nm, obj):
    env = {**os.environ}
    root = os.environ.get("GCC_M68K", "")
    if root:
        env["LD_LIBRARY_PATH"] = f"{root}/usr/lib/x86_64-linux-gnu:" + env.get("LD_LIBRARY_PATH", "")
    out = subprocess.run([nm, "-S", "--defined-only", str(obj)], capture_output=True, text=True, env=env, check=True).stdout
    sizes = {}
    for line in out.splitlines():
        parts = line.split()
        if len(parts) == 4 and parts[2] in "Tt":
            sizes[parts[3]] = int(parts[1], 16)
    return sizes


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--llc", default=str(ROOT / "target/debug/llrm-m68k-llc"))
    ap.add_argument("--nm")
    ap.add_argument("--dir", default=str(ROOT / "crates/target/llrm-m68k/tests/data_size"))
    ap.add_argument("--levels", default="O2,Os")
    ap.add_argument("--corpus", action="store_true", help="every tests/run/c program, from C")
    ap.add_argument("--cc", help="llrm-cc, for --corpus")
    ap.add_argument("--jobs", type=int, default=4)
    args = ap.parse_args()
    nm = args.nm or f"{os.environ['GCC_M68K']}/usr/bin/m68k-linux-gnu-nm"
    if args.corpus:
        sys.path.insert(0, str(ROOT / "tools"))
        import corpus_targets as ct, llrmbin, o0_sweep
    scratch = tempfile.TemporaryDirectory()

    def sources(cpu):
        """(name, .ll path) for `cpu`."""
        if not args.corpus:
            return [(f.stem, f) for f in sorted(Path(args.dir).glob("*.ll"))]
        target, found = f"m68k-{cpu}", []
        for program in sorted((ROOT / "tests/run/c").glob("*.c")):
            text = program.read_text()
            if not ct.applies(target, text):
                continue
            out = Path(scratch.name) / f"{program.stem}-{cpu}.ll"
            cmd = [args.cc, "-S", "-emit-llvm", "-w", *o0_sweep.flags_of(text, target), "-I", str(ROOT / "tests/run/c"),
                   "-I", str(ROOT / "runtime/shared/posix"), "-o", str(out), str(program)]
            env = {**os.environ, **llrmbin.cc_environment(), "LLRM_CC_TARGET": target}
            if subprocess.run(cmd, capture_output=True, text=True, env=env).returncode == 0 and out.exists():
                found.append((program.stem, out))
        return found

    def one(job):
        cpu, level, name, file = job
        with tempfile.TemporaryDirectory() as tmp:
            obj, est = Path(tmp) / "o.o", Path(tmp) / "e.tsv"
            run = subprocess.run(
                [args.llc, "-c", f"-mcpu={cpu}", f"-{level}", f"--size-estimate={est}", "-o", str(obj), str(file)],
                capture_output=True,
                text=True,
            )
            if run.returncode != 0:
                return []  # a program the backend refuses is not a data point
            sizes = function_sizes(nm, obj)
            return [
                (f"{name}:{fn}:{level}", int(e), sizes[fn])
                for fn, e in (line.split("\t") for line in est.read_text().splitlines())
                if fn in sizes and int(e) > 0 and sizes[fn] > 0
            ]

    for cpu in CPUS:
        jobs = [(cpu, level, name, file) for level in args.levels.split(",") for name, file in sources(cpu)]
        with ThreadPoolExecutor(args.jobs) as pool:
            rows = [row for part in pool.map(one, jobs) for row in part]  # (key, estimate, object)
        hold = [r for r in rows if zlib.crc32(r[0].encode()) % 5 == 0]
        train = [r for r in rows if zlib.crc32(r[0].encode()) % 5 != 0]
        scale = statistics.median(o / e for _, e, o in train)
        err = lambda s: statistics.median(abs(e * s - o) / o for _, e, o in hold)
        agg = sum(e for _, e, _ in rows) / sum(o for _, _, o in rows)
        print(
            f"{cpu}: {len(train)} train / {len(hold)} hold-out functions; median object/estimate on train {scale:.3f} "
            f"-> function_scale {round(scale * 100)}; hold-out median |error| {err(scale):.3f} at that scale, {err(1.0):.3f} at 1.0; "
            f"aggregate estimate(raw)/objects {agg:.3f}"
        )


if __name__ == "__main__":
    main()
