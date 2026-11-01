#!/usr/bin/env bash
# usage: [LLRM_QB_FLAGS="-O2 ..."] build.sh PROGRAM.BAS OUT.wasm [OBJDIR]
# A QB program for the headless device as one wasm32 module: llrm-qb for the program, clang and llrm-wasm-llc
# for the runtime, wasm-ld to link. Needs the llrm-qb and llrm-wasm-llc binaries (cargo build -p llrm --bin
# llrm-qb; cargo build -p llrm-wasm --bin llrm-wasm-llc) and clang with the wasm32 target.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
bas=$1; out=$2; obj=${3:-$(mktemp -d)}
qb=${LLRM_QB:-$root/target/debug/llrm-qb}; llc=${LLRM_WASM_LLC:-$root/target/debug/llrm-wasm-llc}
mkdir -p "$obj"
# What tests/qb_arm64.rs links: the portable runtime and the posix glue, the headless device and the host video, and
# this directory's own OS layer.
sources=$(cd "$root" && ls runtime/qb/*.c)
sources="$sources runtime/qb/posix/startup.c runtime/qb/posix/block.c runtime/qb/posix/ad.c runtime/qb/posix/mathops.c
  runtime/qb/posix/paintq.c runtime/qb/posix/module_exports.c runtime/qb/posix/blockentries.c
  runtime/qb/posix/onerror_host.c runtime/qb/posix/program.c runtime/qb/posix/real.c
  runtime/qb/dev/headless/headless.c runtime/qb/dev/host/video.c runtime/qb/dev/host/vgadata.c
  runtime/qb/dev/host/tick.c runtime/wasm/libc.c runtime/wasm/os.c runtime/wasm/main.c"
failed=0
for s in $sources; do
  n=$(echo "$s" | tr / _)
  clang -target wasm32-unknown-unknown -O0 -ffreestanding -nostdinc -isystem "$root/runtime/wasm/include" \
    -isystem "$(clang -print-resource-dir)/include" -fvisibility=default -std=gnu11 -w -D__cdecl= "-DQB_IDLE()=llrm_host_poll()" -include "$root/runtime/wasm/idle.h" -emit-llvm -S \
    -Xclang -disable-llvm-passes -I "$root/runtime/qb/posix" -I "$root/runtime/qb" -I "$root/runtime/qb/dev" \
    -I "$root/runtime/qb/dev/headless" -I "$root/runtime/qb/dev/host" -I "$root/runtime/shared/posix" \
    "$root/$s" -o "$obj/$n.ll"
  "$llc" -filetype=obj "$obj/$n.ll" -o "$obj/$n.o" || { echo "llc failed: $s" >&2; failed=1; }
done
[ $failed = 0 ] || exit 1
"$qb" "$bas" --wasm32 ${LLRM_QB_FLAGS:-} -o "$obj/program.o"
wasm-ld --no-entry --allow-undefined --wrap=dev_key_ready --wrap=dev_clock --wrap=dev_tone --export=llrm_wasm_start --export=llrm_wasm_stack_overflow --export=__wasm_call_ctors --export=headless_type \
  --export=host_pixels --export=host_colors --export=host_tick_fire --export=host_width --export=host_height --export=host_mode --export=host_text_row --export=headless_type_extended \
  --export=__heap_base -z stack-size=8388608 "$obj"/*.o -o "$out"
