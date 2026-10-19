#!/usr/bin/env bash
# Builds tools/scev/oracle.cpp against the pinned LLVM's static libraries
# (~/scratch/llvm-build) into ${1:-~/scratch/scev-oracle/oracle}.
set -euo pipefail
out=${1:-$HOME/scratch/scev-oracle/oracle}
src=$HOME/scratch/llvm-src/llvm
lib=$HOME/scratch/llvm-build/lib
mkdir -p "$(dirname "$out")"
c++ -std=c++17 -O1 -fno-rtti -I "$src/include" -I "$HOME/scratch/llvm-build/include" \
  "$(dirname "$0")/oracle.cpp" -o "$out" \
  -Wl,--start-group "$lib"/libLLVM*.a -Wl,--end-group -lz -llzma -lpthread -ldl -lrt
echo "$out"
