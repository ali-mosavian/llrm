#!/usr/bin/env bash
# tools/targets/arm64-glibc-link.sh OUT OBJECT...: a static arm64 Linux program against the glibc sysroot (LLRM_ARM64_SYSROOT, default
# ~/.cache/llrm-arm64-sysroot/root: the Debian/Ubuntu *-arm64-cross packages unpacked), linked by the pinned build's lld.
set -euo pipefail
root=${LLRM_ARM64_SYSROOT:-$HOME/.cache/llrm-arm64-sysroot/root}
build=${LLVM_BUILD:-$HOME/scratch/llvm-build-aarch64}
lld=$build/bin/ld.lld
[ -x "$lld" ] || lld=ld.lld
lib=$root/usr/aarch64-linux-gnu/lib
gcc=$(echo "$root"/usr/lib/gcc-cross/aarch64-linux-gnu/*)
out=$1; shift
exec "$lld" -static --sysroot="$root" -o "$out" "$lib/crt1.o" "$lib/crti.o" "$gcc/crtbeginT.o" "$@" \
  --start-group "$lib/libc.a" "$lib/libm.a" "$gcc/libgcc.a" "$gcc/libgcc_eh.a" --end-group "$gcc/crtend.o" "$lib/crtn.o"
