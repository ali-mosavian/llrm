#!/bin/bash
# Measure the twins (C written with the arithmetic of Nib's fixed-point `*`, 64-bit products) beside the originals.
# usage: measure.sh OUT.json   (LLRM_BIN, LLRM_OW, LLRM_CLANG_M16 set as for bench.py)
set -e
here=$(cd "$(dirname "$0")" && pwd); bench=$(dirname "$here")
for d in "$here"/*/; do n=$(basename "$d"); rm -rf "$bench/$n"; cp -r "$d" "$bench/$n"; done
trap 'for d in "$here"/*/; do rm -rf "$bench/$(basename "$d")"; done' EXIT
cd "$bench/.." && uv run --project tools python tools/bench/bench.py --no-judge nbfx64 nbody_fixed stftfx64 stft_fixed jpegfx64 jpeg_fixed --lang c nib --json "${1:-twins.json}" --work "${2:-/tmp/twins-work}"
