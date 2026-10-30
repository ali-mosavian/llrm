#!/usr/bin/env bash
# Builds the QB host runtime with llrm-cc (pin front end + llrm-arm64 backend), one object per source.
root=/Users/alim/work/personal/llrm-mac2; qb=$root/runtime/qb; out=$1; mkdir -p $out
export LLRM_CC_TARGET=arm64-macos LLRM_PIN_CLANG=$HOME/work/other/llvm-pin/build/bin/clang
sources=("$qb"/*.c "$qb"/dev/sdl/*.c "$qb"/dev/host/{video,vgadata,tick}.c "$root"/runtime/shared/posix/os.c
  "$qb"/posix/{startup,block,ad,mathops,paintq,module_exports,blockentries,onerror_host,program,real}.c)
SDK=$(xcrun --show-sdk-path); SDL=/opt/homebrew/Cellar/sdl2/2.32.10
for s in "${sources[@]}"; do
  o=$out/$(echo ${s#$root/} | tr / _).o
  $root/target/release/llrm-cc -O2 -std=gnu11 -w -g0 -pthread -D__cdecl= -isysroot $SDK -I$SDL/include -I$SDL/include/SDL2 \
    -I $qb/posix -I $qb -I $qb/dev -I $qb/dev/sdl -I $qb/dev/host -I $root/runtime/shared/posix -c $s -o $o 2>$o.err || echo "FAIL $s: $(head -c 300 $o.err)"
done
