#!/usr/bin/env bash
# Regenerates crates/target/llrm-arm64/tests/darwin from tests/programs: each
# program's IR for arm64-apple-macos (the pinned clang's front end alone) and
# the output the pinned `clang -O2` build prints under qemu-aarch64 on Linux,
# which the Mac checks its run against (tools/targets/arm64-mac-check.sh).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
crate=$root/crates/target/llrm-arm64
build=${LLVM_BUILD:-$HOME/scratch/llvm-build-aarch64}
tmp=$(mktemp -d)
"$build/bin/clang" -target aarch64-unknown-linux-gnu -O2 -ffreestanding -fno-builtin -c -o "$tmp/rt.o" "$crate/tests/rt/rt.c"
for source in "$crate"/tests/programs/*.c; do
  name=$(basename "$source" .c)
  "$build/bin/clang" -target arm64-apple-macos -O2 -Xclang -disable-llvm-passes -Xclang -disable-lifetime-markers \
    -S -emit-llvm -o "$crate/tests/darwin/$name.ll" "$source"
  # No path of this checkout in the fixture.
  sed -i -e '/^; ModuleID/d' -e 's|^source_filename = ".*/|source_filename = "|' "$crate/tests/darwin/$name.ll"
  # -fsigned-char: Linux AArch64 defaults to unsigned char, the Darwin IR above to signed.
  "$build/bin/clang" -target aarch64-unknown-linux-gnu -fsigned-char -O2 -c -o "$tmp/$name.o" "$source"
  "$build/bin/ld.lld" -static -o "$tmp/$name" "$tmp/$name.o" "$tmp/rt.o"
  set +e
  qemu-aarch64 "$tmp/$name" > "$crate/tests/darwin/$name.out"
  echo $? > "$crate/tests/darwin/$name.status"
  set -e
done
