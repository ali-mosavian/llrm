#!/bin/bash
# m16 -O2: the f32 files in one translation unit (the shape of the sfm16 kernels) must not exceed main's runtime/softfloat sf32.c + sf32_conv.c (2758 B, release/next 2026-11-03).
# With SF_PACK_INLINE in the m16 -O2 config the kernels were 19 B over main in all three cells. (Per-file objects sum higher by design: each repeats its static helpers; linked reach per entry is what m16_split_padding.sh prices.)
cd "$(dirname "$0")/.." || exit 1
export LLRM_CC_TARGET=x86_16-wat; D=${LLRM_CC_DIR:-$HOME/targets/gs-out/rbase}; CAP=2758; T=$(mktemp -d); trap 'rm -r $T' EXIT
for p in $(tests/fp_sources.sh f32); do python3 tests/flatten.py $p; done > $T/all.c
$D/llrm-cc -march=i486 -O2 -include config/x86_m16/config.h -Iinclude -c $T/all.c -o $T/x.obj || exit 2
tot=$(python3 tests/omf_code_bytes.py $T/x.obj)
echo "m16 -O2 f32 one-TU object $tot B (cap $CAP)"; [ $tot -le $CAP ] || { echo FAIL; exit 1; }
