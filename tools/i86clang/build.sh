#!/bin/sh
# Build i86clang against the installed LLVM (llvm-config on PATH or LLVM_BIN): tools/i86clang/build.sh [output]
set -eu
HERE="$(cd "$(dirname "$0")" && pwd)"
BIN="${LLVM_BIN:-$(dirname "$(command -v llvm-config 2>/dev/null || echo /opt/homebrew/opt/llvm/bin/llvm-config)")}"
OUT="${1:-$HERE/build/i86clang}"
mkdir -p "$(dirname "$OUT")"
# clang's headers need exceptions and RTTI settings llvm-config does not give a tool that links libclang-cpp
"$BIN/clang++" -std=c++17 -fno-rtti $("$BIN/llvm-config" --cxxflags | sed 's/-fno-exceptions//') "$HERE/i86clang.cpp" -o "$OUT" \
  $("$BIN/llvm-config" --ldflags) -lclang-cpp -lLLVM
