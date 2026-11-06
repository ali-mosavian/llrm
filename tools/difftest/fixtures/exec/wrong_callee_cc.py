#!/usr/bin/env python3
"""A stand-in for llrm-cc that compiles like clang -O0 but calls c0 where the source calls c1: the symptom of #6146 (a merged sibling call)."""
import os
import subprocess
import sys

args = sys.argv[1:]
source, out = args[args.index("-c") + 1], args[args.index("-o") + 1]
text = open(source).read().replace("c1(", "c0(")
patched = out + ".c"
open(patched, "w").write(text)
clang = os.environ.get("LLRM_PIN_CLANG", os.path.expanduser("~/scratch/llvm-build/bin/clang"))
sys.exit(subprocess.run([clang, "-O0", "-c", "-fno-pic", "-w", patched, "-o", out]).returncode)
