#!/usr/bin/env bash
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel); S=$(dirname "$0"); qb=$root/runtime/qb; mkdir -p $S/hl
export LLRM_CC_TARGET=arm64-macos LLRM_PIN_CLANG=${LLRM_PIN_CLANG:?set to the pin clang}
SDK=$(xcrun --show-sdk-path)
fl=(-std=gnu11 -w -pthread -D__cdecl= -I $qb/posix -I $qb -I $qb/dev -I $qb/dev/headless -I $qb/dev/host -I $root/runtime/shared/posix)
for f in headless main; do
  $root/target/release/llrm-cc -O2 "${fl[@]}" -isysroot $SDK -c $qb/dev/headless/$f.c -o $S/hl/$f.o 2>$S/hl/$f.err && echo "llrm-cc $f" || { clang -O2 "${fl[@]}" -c $qb/dev/headless/$f.c -o $S/hl/$f.o && echo "clang $f: $(head -c 200 $S/hl/$f.err)"; }
done
objs=$(ls $S/rt/*.o $S/sys/*.o | grep -v dev_sdl)
clang -o $S/gorilla_headless $objs $S/hl/*.o $S/GORILLA.darwin.o -lm -pthread 2>&1 | grep -v "platform load command" | head
QB_HEADLESS_SECONDS=6 QB_HEADLESS_KEYS='\r\r\r\r' QB_HEADLESS_PPM=$S/g.ppm timeout 20 $S/gorilla_headless | tail -30; echo "exit ${PIPESTATUS[0]}"; ls -la $S/g.ppm
