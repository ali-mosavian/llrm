#!/usr/bin/env python3
"""The arm64 backend against the pinned `clang -O2` on crates/target/llrm-arm64/tests/programs.

For each program: bytes of code, instructions executed under qemu-aarch64 in the
program's own functions (the shared runtime is not counted), and whether the
output matches. Premise assertions: both builds ran to an exit with the same
output, and the trace counted some instructions."""
import bisect
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATE = ROOT / "crates/target/llrm-arm64"
BUILD = Path(os.environ.get("LLVM_BUILD", Path.home() / "scratch/llvm-build-aarch64"))
CLANG, LLD, OBJDUMP = BUILD / "bin/clang", BUILD / "bin/ld.lld", BUILD / "bin/llvm-objdump"
LLC = ROOT / "target/debug/llrm-arm64-llc"
TRIPLE = "aarch64-unknown-linux-gnu"
RUNTIME_FUNCTIONS = {"_start", "exit", "write", "strlen", "memcpy", "memset", "putchar", "puts", "number", "printf"}


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


def code_bytes(binary):
    return sum(size for address, size, name in symbols(binary) if name not in RUNTIME_FUNCTIONS)


def executed(binary):
    """Instructions executed in the program's own functions, from a trace with
    one instruction per translation block."""
    log = tempfile.NamedTemporaryFile(suffix=".log", delete=False).name
    done = subprocess.run(
        ["qemu-aarch64", "-one-insn-per-tb", "-d", "exec,nochain", "-D", log, str(binary)], capture_output=True
    )
    table = symbols(binary)
    starts = [entry[0] for entry in table]
    count = 0
    for line in open(log):
        match = re.match(r"Trace \d+: \S+ \[[0-9a-f]+/([0-9a-f]+)/", line)
        if not match:
            continue
        pc = int(match[1], 16)
        at = bisect.bisect_right(starts, pc) - 1
        if at >= 0 and table[at][2] not in RUNTIME_FUNCTIONS and pc < table[at][0] + table[at][1]:
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


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "whole"
    assert mode in ("whole", "backend"), "usage: arm64_compare.py [whole|backend]"
    runtime = tempfile.mkdtemp() + "/rt.o"
    run(CLANG, "-target", TRIPLE, "-O2", "-ffreestanding", "-fno-builtin", "-c", "-o", runtime, CRATE / "tests/rt/rt.c")
    print(f"{'program':12} {'code ours':>10} {'code clang':>10} {'exec ours':>10} {'exec clang':>10}  output")
    totals = [0, 0, 0, 0]
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
        print(f"{source.stem:12} {row[0]:10} {row[1]:10} {row[2]:10} {row[3]:10}  {'same' if same else 'DIFFERENT'}")
    print(f"{'total':12} {totals[0]:10} {totals[1]:10} {totals[2]:10} {totals[3]:10}")
    print(f"ratio: code {totals[0] / totals[1]:.2f}x, executed {totals[2] / totals[3]:.2f}x")


main()
