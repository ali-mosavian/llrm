#!/bin/sh
# Builds apfloat_dump.cpp against the LLVM build of d1106deb71cc and runs it; the output is the golden file.
B=${LLVM_BUILD:-$HOME/scratch/llvm-build}
S=${LLVM_SRC:-$HOME/scratch/llvm-src}
T=$(mktemp -d)
"$B/bin/clang++" --version | head -1 >&2
"$B/bin/clang++" -std=c++17 -O1 -I"$S/llvm/include" -I"$B/include" "$(dirname "$0")/apfloat_dump.cpp" \
  -L"$B/lib" -lLLVMSupport -lLLVMDemangle -lpthread -lz -o "$T/dump" >&2 && "$T/dump" "$@"
