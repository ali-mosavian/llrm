#!/usr/bin/env bash
# On an arm64 Mac: compiles the QB host runtime with llrm-cc (the pin's front end and llrm-arm64-llc),
# one object per source, into $1 (default: a temp dir). Prints each source that fails.
#   LLRM_PIN_CLANG=/path/to/pin/clang tools/targets/qb-runtime-check.sh [outdir]
set -uo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd); qb=$root/runtime/qb
out=${1:-$(mktemp -d)}; mkdir -p "$out"
export LLRM_CC_TARGET=arm64-macos
cargo build --release -q -p llrm-cc -p llrm-arm64 || exit 1
cc=${CARGO_TARGET_DIR:-$root/target}/release/llrm-cc
sdk=$(xcrun --show-sdk-path); sdl=$(brew --prefix sdl2)
sources=("$qb"/*.c "$qb"/dev/sdl/*.c "$qb"/dev/host/{video,vgadata,tick}.c "$root"/runtime/shared/posix/os.c
  "$qb"/posix/{startup,block,ad,mathops,paintq,module_exports,blockentries,onerror_host,program,real}.c)
failed=0
for s in "${sources[@]}"; do
  o=$out/$(echo "${s#$root/}" | tr / _).o
  "$cc" -O2 -std=gnu11 -w -g0 -pthread -D__cdecl= -isysroot "$sdk" -I"$sdl/include" -I"$sdl/include/SDL2" \
    -I "$qb/posix" -I "$qb" -I "$qb/dev" -I "$qb/dev/sdl" -I "$qb/dev/host" -I "$root/runtime/shared/posix" \
    -c "$s" -o "$o" 2>"$o.err" || { echo "FAIL ${s#$root/}: $(head -c 200 "$o.err" | head -1)"; failed=1; }
done
exit $failed
