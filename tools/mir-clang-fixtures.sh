#!/usr/bin/env bash
# Regenerates crates/llrm-mir/tests/clang from bench/parity/*.c.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
out=$root/crates/llrm-mir/tests/clang
for file in "$root"/bench/parity/*.c; do
  name=$(basename "$file" .c)
  clang --target=msp430 -O1 -S -emit-llvm -fno-discard-value-names -o - "$file" 2>/dev/null \
    | grep -v '^target triple' | sed 's/ dso_local//g' > "$out/$name.ll" || rm -f "$out/$name.ll"
done
