#!/bin/bash
# Shadow coverage over the arm64 C programs: each function the DAG path refused, with its reason.
# Needs LLRM_PIN_CLANG (the pin's clang). Prints "fallbacks: N panics: M"; today N=1 (vla @main).
cd "$(dirname "$0")/.."
export LLRM_PIN_CLANG=${LLRM_PIN_CLANG:-$HOME/work/other/llvm-pin/build/bin/clang}
out=$(mktemp -d)
cargo build -q -p llrm-cc -p llrm-arm64 || exit 1
: > "$out/all.txt"
for f in crates/target/llrm-arm64/tests/programs/*.c; do
  n=$(basename "$f" .c)
  LLRM_ISEL_SHADOW=1 target/debug/llrm-cc -c -O2 -target arm64-apple-macosx "$f" -o "$out/$n.o" 2>&1 \
    | grep -E "isel-shadow|panicked" | sed "s/^/$n: /" >> "$out/all.txt"
done
cut -c1-170 "$out/all.txt"
echo "fallbacks: $(grep -c isel-shadow "$out/all.txt") panics: $(grep -c panicked "$out/all.txt") ($(ls crates/target/llrm-arm64/tests/programs/*.c | wc -l | tr -d ' ') programs)"
