#!/usr/bin/env bash
# Links a QB module object (llrm-qb --arm64=darwin|linux) with the host runtime on the sdl device platform.
# usage: tools/link-qb-sdl.sh prog.o out     (SDL2 from sdl2-config; run `out` for a window)
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
qb=$root/runtime/qb
sources=("$qb"/*.c "$qb"/dev/sdl/*.c "$qb"/dev/host/{video,vgadata,tick}.c "$root"/runtime/shared/posix/os.c
    "$qb"/posix/{startup,block,ad,mathops,paintq,module_exports,blockentries,onerror_host,program,real}.c)
# shellcheck disable=SC2046
cc -std=gnu11 -Os -w -g -pthread -D__cdecl= $(sdl2-config --cflags) \
    -I "$qb/posix" -I "$qb" -I "$qb/dev" -I "$qb/dev/sdl" -I "$qb/dev/host" -I "$root/runtime/shared/posix" \
    "${sources[@]}" "$1" $(sdl2-config --libs) -lm -o "$2"
