#!/bin/sh
# include/softfloat.h and sf_helpers.h must parse for every clang target the runtime is built for (a freestanding include of each). The binary128 addition used
# __float128 by default: clang rejects it on aarch64 ("__float128 is not supported on this target"), which would have stopped every soft-float file on arm64.
# usage: [CLANG=pin clang with all targets] tests/softfloat_h_targets.sh
cd "$(dirname "$0")/.." || exit 1
C=${CLANG:-$HOME/scratch/llvm-build-clang-wasm/bin/clang}
printf '#include "softfloat.h"\n#include "sf_helpers.h"\nint f(sf32 a) { return (int)a; }\n' > "${TMPDIR:-/tmp}/sf_h_targets.c"
rc=0; n=0
for t in wasm32 aarch64-linux-gnu aarch64-apple-darwin x86_64-linux-gnu i386-linux-gnu i386-unknown-unknown-code16 m68k-unknown-linux-gnu; do
  out=$($C --target=$t -Iinclude -ffreestanding -fsyntax-only "${TMPDIR:-/tmp}/sf_h_targets.c" 2>&1)
  case $out in *"unknown target"*|*"No available targets"*|*"unsupported"*) continue;; esac
  n=$((n + 1)); [ -z "$out" ] || { echo "$t: $(echo "$out" | head -1)"; rc=1; }
done
rm -f "${TMPDIR:-/tmp}/sf_h_targets.c"
[ $n -gt 0 ] || { echo "no target was usable: is $C a clang?"; exit 2; }
[ $rc = 0 ] && echo "softfloat.h parses on $n targets"; exit $rc
