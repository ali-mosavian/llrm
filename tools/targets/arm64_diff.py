#!/usr/bin/env python3
"""Static mnemonic histogram of this backend's code against the pinned llc -O2 on the same
clang -O2 IR, per directory of programs. The
differences, biggest first, are the isel patterns this backend lacks.
Usage: tools/targets/arm64_diff.py [programs-dir ...]"""
import collections
import re
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import arm64_compare as c


def mnemonics(obj):
    """{function: [mnemonic, ...]} from the disassembly of an object."""
    out = c.run(c.OBJDUMP, "-d", "--no-show-raw-insn", obj)
    functions, current = {}, None
    for line in out.splitlines():
        head = re.match(r"[0-9a-f]+ <(.+)>:", line)
        if head:
            current = functions.setdefault(head[1], [])
        elif current is not None and (m := re.match(r"\s+[0-9a-f]+:\s+(\S+)", line)):
            current.append(m[1])
    return functions


def main():
    dirs = [Path(a) for a in sys.argv[1:]] or [c.CRATE / "tests/programs", c.CRATE / "tests/kernels"]
    for directory in dirs:
        ours, theirs = collections.Counter(), collections.Counter()
        for source in sorted(directory.glob("*.c")):
            work = Path(tempfile.mkdtemp())
            ir = work / "p.ll"
            c.run(c.CLANG, "-target", c.TRIPLE, "-O2", "-fno-vectorize", "-fno-slp-vectorize", "-fno-unroll-loops",
                  "-Xclang", "-disable-lifetime-markers", "-S", "-emit-llvm", "-o", ir, source)
            try:
                c.run(c.LLC, "--no-opt", "-o", work / "o.o", ir)
            except SystemExit:
                continue
            c.run(c.BUILD / "bin/llc", "-mtriple=" + c.TRIPLE, "-O2", "-filetype=obj", "-o", work / "l.o", ir)
            a, b = mnemonics(work / "o.o"), mnemonics(work / "l.o")
            # Our objects carry no local function symbols, so only whole objects compare.
            ours.update(m for f in a.values() for m in f)
            theirs.update(m for f in b.values() for m in f)
        print(f"== {directory.name}: {sum(ours.values())} ours, {sum(theirs.values())} llc instructions")
        rows = sorted(((ours[m] - theirs[m], m) for m in set(ours) | set(theirs)), reverse=True)
        print("  more in ours: ", ", ".join(f"{m} {d:+}" for d, m in rows[:14] if d > 0))
        print("  fewer in ours:", ", ".join(f"{m} {d:+}" for d, m in rows[::-1][:14] if d < 0))

if __name__ == "__main__":
    main()
