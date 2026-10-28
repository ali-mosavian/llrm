#!/usr/bin/env bash
# tools/targets/arm64-link.sh OUT OBJECT...: a static arm64 Linux program for qemu-aarch64: the objects and the freestanding runtime
# the arm64 tests use, built once by the pinned clang with the AArch64 backend (LLVM_BUILD, default llvm-build-aarch64).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
build=${LLVM_BUILD:-$HOME/scratch/llvm-build-aarch64}
cache=${XDG_CACHE_HOME:-$HOME/.cache}/llrm-arm64-rt
mkdir -p "$cache"
rt=$cache/rt.o
if [ ! -s "$rt" ] || [ "$rt" -ot "$root/crates/target/llrm-arm64/tests/rt/rt.c" ]; then
  "$build/bin/clang" -target aarch64-unknown-linux-gnu -O2 -ffreestanding -fno-builtin -c \
    -o "$rt.$$" "$root/crates/target/llrm-arm64/tests/rt/rt.c"
  mv "$rt.$$" "$rt"
fi
out=$1; shift
exec "$build/bin/ld.lld" -static -o "$out" "$@" "$rt"
