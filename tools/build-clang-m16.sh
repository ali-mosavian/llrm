#!/usr/bin/env bash
# tools/build-clang-m16.sh: the pinned clang plus toolchain/clang/patches (the i8086 target), built front end only.
# Prints the clang path. Env: LLVM_SRC (a checkout holding the pin, default ~/scratch/llvm-src), M16_DIR (default ~/scratch).
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
pin=$(tr -d '[:space:]' <"$root/toolchain/clang/pin")
src=${LLVM_SRC:-$HOME/scratch/llvm-src}
base=${M16_DIR:-$HOME/scratch}
tree=$base/llvm-src-m16
build=$base/llvm-build-m16
if [ ! -d "$tree" ]; then
  git -C "$src" worktree add --detach "$tree" "$pin" >&2
  for patch in "$root"/toolchain/clang/patches/*.patch; do git -C "$tree" apply "$patch" >&2; done
fi
[ -f "$build/build.ninja" ] || cmake -G Ninja -S "$tree/llvm" -B "$build" -DCMAKE_BUILD_TYPE=Release \
  -DLLVM_ENABLE_PROJECTS=clang -DLLVM_TARGETS_TO_BUILD=X86 -DLLVM_USE_LINKER=lld \
  -DCMAKE_CXX_FLAGS_RELEASE="-O1 -DNDEBUG" -DCMAKE_C_FLAGS_RELEASE="-O1 -DNDEBUG" \
  -DLLVM_INCLUDE_TESTS=OFF -DLLVM_INCLUDE_BENCHMARKS=OFF -DCLANG_INCLUDE_TESTS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_PARALLEL_LINK_JOBS=2 ${CMAKE_EXTRA:-} >&2
nice -n 19 ninja -C "$build" clang >&2
echo "$build/bin/clang"
