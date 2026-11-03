#!/bin/bash
# m16 -O2: library object bytes (f32 files) must not exceed main's runtime/softfloat sf32.c + sf32_conv.c (2758 B, release/next 2026-11-03).
# With SF_PACK_INLINE in the m16 -O2 config the sum was 2812 (+54) and the sfm16 kernels were 19 B over main in all three cells.
cd "$(dirname "$0")/.." || exit 1
export LLRM_CC_TARGET=x86_16-wat; D=${LLRM_CC_DIR:-$HOME/targets/gs-out/rbase}; CAP=2758; tot=0; T=$(mktemp -d); trap 'rm -r $T' EXIT
for p in $(tests/fp_sources.sh f32); do $D/llrm-cc -march=i486 -O2 -include config/x86_m16/config.h -Iinclude -c $p -o $T/x.obj || exit 2; tot=$((tot + $(python3 tests/omf_code_bytes.py $T/x.obj))); done
echo "m16 -O2 f32 objects $tot B (cap $CAP)"; [ $tot -le $CAP ] || { echo FAIL; exit 1; }
