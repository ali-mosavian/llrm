#!/usr/bin/env python3
"""Where llc's advantage comes from: the pinned `llc -O2` on clang's -O2 IR with one pass
switched off at a time, executed instructions summed over tests/programs. A pass's
contribution is how much worse llc gets without it; ours is the same IR through this
backend. Usage: tools/targets/arm64_attribute.py [programs-dir]"""
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import arm64_compare as c

OFF = [
    ("ldst-opt (ldp/stp)", ["-aarch64-enable-ldst-opt=0"]),
    ("ccmp", ["-aarch64-enable-ccmp=0"]),
    ("machine-combiner", ["-aarch64-enable-mcr=0"]),
    ("cond-br-tune", ["-aarch64-enable-cond-br-tune=0"]),
    ("condopt", ["-aarch64-enable-condopt=0"]),
    ("copyelim", ["-aarch64-enable-copyelim=0"]),
    ("sink-fold", ["-aarch64-enable-sink-fold=0"]),
    ("dead-defs", ["-aarch64-enable-dead-defs=0"]),
    ("lsr", ["-disable-lsr"]),
    ("machine-sink", ["-disable-machine-sink"]),
    ("machine-cse", ["-disable-machine-cse"]),
    ("machine-licm", ["-disable-machine-licm"]),
    ("peephole", ["-disable-peephole"]),
    ("copyprop", ["-disable-copyprop"]),
    ("block-placement", ["-disable-block-placement"]),
    ("misched", ["-enable-misched=0", "-enable-post-misched=0"]),
    ("early-ifcvt", ["-aarch64-enable-early-ifcvt=0"]),
    ("select-opt", ["-aarch64-select-opt=0"]),
]


def main():
    runtime = tempfile.mkdtemp() + "/rt.o"
    c.run(c.CLANG, "-target", c.TRIPLE, "-O2", "-ffreestanding", "-fno-builtin", "-c", "-o", runtime, c.CRATE / "tests/rt/rt.c")
    sums = {name: 0 for name in ["ours", "llc"] + [n for n, _ in OFF]}
    sizes = dict(sums)
    changed = dict(sums)  # programs whose asm differs from the baseline llc's: 0 means the flag did nothing
    for source in sorted(Path(sys.argv[1] if len(sys.argv) > 1 else c.CRATE / "tests/programs").glob("*.c")):
        work = Path(tempfile.mkdtemp())
        ir = work / "p.ll"
        c.run(c.CLANG, "-target", c.TRIPLE, "-O2", "-fno-vectorize", "-fno-slp-vectorize", "-fno-unroll-loops",
              "-Xclang", "-disable-lifetime-markers", "-S", "-emit-llvm", "-o", ir, source)
        try:
            c.run(c.LLC, "--no-opt", "-o", work / "ours.o", ir)
        except SystemExit:
            continue  # the reader lacks a construct; skipped for every column
        base_text = c.run(c.BUILD / "bin/llc", "-mtriple=" + c.TRIPLE, "-O2", "-o", "-", ir)
        variants = {"ours": None, "llc": []} | {n: flags for n, flags in OFF}
        for name, flags in variants.items():
            if flags is not None:
                text = c.run(c.BUILD / "bin/llc", "-mtriple=" + c.TRIPLE, "-O2", *flags, "-o", "-", ir)
                base = base_text if name != "llc" else text
                changed[name] += text != base
                c.run(c.BUILD / "bin/llc", "-mtriple=" + c.TRIPLE, "-O2", *flags, "-filetype=obj", "-o", work / f"{name[:8]}.o", ir)
            obj = work / ("ours.o" if flags is None else f"{name[:8]}.o")
            exe = work / f"{name[:8]}.exe"
            c.run(c.LLD, "-static", "-o", exe, obj, runtime)
            out, status, count = c.executed(exe)
            sums[name] += count
            sizes[name] += c.code_bytes(exe)
    base = sums["llc"]
    print(f"{'llc without':22} {'executed':>10} {'vs llc':>7} {'bytes':>7} {'asm differs':>11}")
    for name, total in sorted(sums.items(), key=lambda kv: kv[1]):
        print(f"{name:22} {total:10} {total / base:7.3f} {sizes[name]:7} {changed[name]:11}")


if __name__ == "__main__":
    main()
