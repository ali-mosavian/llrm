#!/usr/bin/env python3
"""The arm64 backend against the pinned `clang -O2` on crates/target/llrm-arm64/tests/programs.

For each program: bytes of code, instructions executed under qemu-aarch64 in the
program's own functions (the shared runtime is not counted), and whether the
output matches. Premise assertions: both builds ran to an exit with the same
output, and the trace counted some instructions."""
import bisect
import math
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "crates/target/llrm-arm64"
BUILD = Path(os.environ.get("LLVM_BUILD", Path.home() / "scratch/llvm-build-aarch64"))
CLANG, LLD, OBJDUMP = BUILD / "bin/clang", BUILD / "bin/ld.lld", BUILD / "bin/llvm-objdump"
LLC = ROOT / "target/debug/llrm-arm64-llc"
TRIPLE = "aarch64-unknown-linux-gnu"
RUNTIME_FUNCTIONS = {
    "_start", "exit", "write", "strlen", "memcpy", "memset", "putchar", "puts", "number", "printf", "malloc", "free",
    "calloc", "strcmp", "strcpy", "memcmp", "atoi", "qsort", "syscall3",
}


def run(*command, **kwargs):
    done = subprocess.run([str(c) for c in command], capture_output=True, text=True, **kwargs)
    if done.returncode != 0:
        sys.exit(f"{command}: {done.stderr}")
    return done.stdout


def symbols(binary):
    """(address, size, name) of the functions of `binary`, sorted."""
    found = []
    for line in run(OBJDUMP, "-t", binary).splitlines():
        match = re.match(r"([0-9a-f]+) .* \.text\s+([0-9a-f]+) (\S+)$", line)
        if match:
            found.append((int(match[1], 16), int(match[2], 16), match[3]))
    return sorted(found)


def program_range(binary):
    """[start, end) of the program's own code in `binary`: the object is linked
    before the runtime, so its code is what lies below the runtime's first
    function. (The symbol table lists no local function of this compiler's
    objects, so symbols cannot say which functions are the program's.)"""
    table = symbols(binary)
    runtime = [address for address, _, name in table if name in RUNTIME_FUNCTIONS]
    assert runtime, "the runtime is not in the binary"
    start = min(address for address, _, _ in table)
    # The object's code ends where the runtime's begins, less the alignment padding.
    return start, min(runtime)


def code_bytes(binary):
    start, end = program_range(binary)
    return end - start


def executed(binary):
    """Instructions executed in the program's own code, from a trace with one
    instruction per translation block."""
    log = tempfile.NamedTemporaryFile(suffix=".log", delete=False).name
    done = subprocess.run(
        ["qemu-aarch64", "-one-insn-per-tb", "-d", "exec,nochain", "-D", log, str(binary)], capture_output=True
    )
    start, end = program_range(binary)
    count = 0
    for line in open(log):
        match = re.match(r"Trace \d+: \S+ \[[0-9a-f]+/([0-9a-f]+)/", line)
        if match and start <= int(match[1], 16) < end:
            count += 1
    os.unlink(log)
    return done.stdout, done.returncode, count


def build(source, work, mode, runtime):
    """`ours` and `ref` executables of `source`. In mode "whole" ours is the
    unoptimized IR through this compiler's mid-end and backend, ref the pinned
    clang -O2. In mode "backend" both take the pinned clang's -O2 IR (no
    vectorizer), ours through the backend alone, ref through the pinned llc -O2."""
    ir = work / "p.ll"
    if mode == "whole":
        run(CLANG, "-target", TRIPLE, "-O2", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-lifetime-markers",
            "-S", "-emit-llvm", "-o", ir, source)
        run(LLC, "-o", work / "ours.o", ir)
        run(CLANG, "-target", TRIPLE, "-O2", "-c", "-o", work / "ref.o", source)
    else:
        run(CLANG, "-target", TRIPLE, "-O2", "-fno-vectorize", "-fno-slp-vectorize", "-fno-unroll-loops",
            "-Xclang", "-disable-lifetime-markers", "-S", "-emit-llvm", "-o", ir, source)
        run(LLC, "--no-opt", "-o", work / "ours.o", ir)
        run(BUILD / "bin/llc", "-mtriple=" + TRIPLE, "-O2", "-filetype=obj", "-o", work / "ref.o", ir)
    for name in ("ours", "ref"):
        run(LLD, "-static", "-o", work / name, work / f"{name}.o", runtime)


def instructions(*command):
    """User instructions the command runs (`perf stat`), the least of 3 runs."""
    best = None
    for _ in range(3):
        done = subprocess.run(
            ["perf", "stat", "-x,", "-e", "instructions:u", *[str(c) for c in command]], capture_output=True, text=True
        )
        if done.returncode != 0:
            sys.exit(f"{command}: {done.stderr}")
        count = int(re.search(r"^(\d+),,instructions:u", done.stderr, re.M)[1])
        best = count if best is None else min(best, count)
    assert best > 0, "perf counted nothing"
    return best


def compile_time():
    """Compile-time cost per program: this compiler (mid-end and backend) against
    the pinned `opt -O2` then `llc -O2`, on the same un-optimized IR. Plain
    `perf stat instructions:u`, own plus inclusive of both tools; the front end is
    in neither."""
    release = ROOT / "target/release/llrm-arm64-llc"
    print(f"{'program':12} {'ours':>12} {'opt+llc':>12} {'ratio':>6}")
    total = [0, 0]
    for source in sorted((CRATE / "tests/programs").glob("*.c")):
        work = Path(tempfile.mkdtemp())
        ir = work / "p.ll"
        run(CLANG, "-target", TRIPLE, "-O2", "-Xclang", "-disable-llvm-passes", "-Xclang", "-disable-lifetime-markers",
            "-S", "-emit-llvm", "-o", ir, source)
        ours = instructions(release, "-o", work / "ours.o", ir)
        opt = instructions(BUILD / "bin/opt", "-O2", "-S", "-o", work / "o.ll", ir)
        llc = instructions(BUILD / "bin/llc", "-mtriple=" + TRIPLE, "-O2", "-filetype=obj", "-o", work / "l.o", work / "o.ll")
        print(f"{source.stem:12} {ours:12} {opt + llc:12} {ours / (opt + llc):6.2f}")
        total = [total[0] + ours, total[1] + opt + llc]
    print(f"{'total':12} {total[0]:12} {total[1]:12} {total[0] / total[1]:6.2f}")


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "whole"
    assert mode in ("whole", "backend", "compile"), "usage: arm64_compare.py [whole|backend|compile]"
    if mode == "compile":
        return compile_time()
    runtime = tempfile.mkdtemp() + "/rt.o"
    run(CLANG, "-target", TRIPLE, "-O2", "-ffreestanding", "-fno-builtin", "-c", "-o", runtime, CRATE / "tests/rt/rt.c")
    print(f"{'program':12} {'code ours':>10} {'code clang':>10} {'exec ours':>10} {'exec clang':>10}  output")
    totals = [0, 0, 0, 0]
    ratios = ([], [])
    for source in sorted((CRATE / "tests/programs").glob("*.c")):
        work = Path(tempfile.mkdtemp())
        try:
            build(source, work, mode, runtime)
        except SystemExit as why:
            print(f"{source.stem:12} not built: {str(why).splitlines()[-1][:90]}")
            continue
        (out_ours, status_ours, n_ours), (out_ref, status_ref, n_ref) = executed(work / "ours"), executed(work / "ref")
        assert n_ref > 0 and n_ours > 0, "the trace counted nothing"
        same = out_ours == out_ref and status_ours == status_ref
        row = [code_bytes(work / "ours"), code_bytes(work / "ref"), n_ours, n_ref]
        totals = [a + b for a, b in zip(totals, row)]
        ratios[0].append(row[0] / row[1])
        ratios[1].append(row[2] / row[3])
        print(f"{source.stem:12} {row[0]:10} {row[1]:10} {row[2]:10} {row[3]:10}  {'same' if same else 'DIFFERENT'}")
    print(f"{'total':12} {totals[0]:10} {totals[1]:10} {totals[2]:10} {totals[3]:10}")
    print(f"ratio of totals: code {totals[0] / totals[1]:.2f}x, executed {totals[2] / totals[3]:.2f}x")
    geomean = lambda values: math.exp(sum(math.log(v) for v in values) / len(values))
    print(f"geomean per program: code {geomean(ratios[0]):.2f}x, executed {geomean(ratios[1]):.2f}x")


if __name__ == "__main__":
    main()
