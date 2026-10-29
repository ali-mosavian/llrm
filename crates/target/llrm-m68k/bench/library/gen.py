#!/usr/bin/env python3
"""Regenerates NAME.ll from NAME.c: clang -O2 for m68k, then the lines MIR's
subset of LLVM does not take are dropped (module identity, metadata, function
and parameter attributes). The IR is otherwise clang's own."""
import glob
import re
import subprocess

for source in sorted(glob.glob("*.c")):
    name = source[:-2]
    text = subprocess.check_output(
        ["clang", "--target=m68k-linux-gnu", "-mcpu=M68000", "-O2", "-ffreestanding", "-fno-unwind-tables",
         "-fno-asynchronous-unwind-tables", "-S", "-emit-llvm", source, "-o", "-"], text=True)
    out = []
    for line in text.splitlines():
        if re.match(r"(; |source_filename|target triple|attributes #|!)", line) or not line.strip():
            if not line.strip():
                out.append(line)
            continue
        line = re.sub(r", !\w+(\.\w+)* !\d+", "", line)
        line = re.sub(r"\brange\([^)]*\) ", "", line)
        line = re.sub(r"\b(dso_local|local_unnamed_addr|noundef|nocapture|nonnull|readonly|writeonly|nuw|nsw|inbounds|samesign)\b ?", "", line)
        line = re.sub(r" #\d+", "", line)
        line = re.sub(r"\bnoalias\b ?", "", line)
        out.append(line)
    open(name + ".ll", "w").write("\n".join(out).strip() + "\n")
