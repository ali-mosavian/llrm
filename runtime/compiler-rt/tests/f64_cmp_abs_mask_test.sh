#!/bin/bash
# f64 cmp tested NaN and zero with `(a << 1) > (INF << 1)`: a 64-bit shift is shld+shl on a 32-bit x86 and m68k. With SF_WIDE_ABS_MASK (config/x86_m32, config/m68k) it is a mask test:
# cmp.c at m32 -O2 has 0 shld (21 without), 400 compares -O2 48947 -> 42935 clk (-12.3%), -Os 69554 -> 61337; m68k-68000 f64 cmp -Os 330 -> 314 B, -O2 1546 -> 1434. Fails above 0 shld.
cd "$(dirname "$0")/.."
D=${LLRM_CC_DIR:-$HOME/scratch/x86m32-live-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -O2 -S -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h -Iinclude lib/fp/f64/cmp.c -o $T/c.s || exit 2
n=$(grep -c -i shld $T/c.s)
[ "$n" -eq 0 ] || { echo "FAIL: f64 cmp has $n shld at m32 -O2 (limit 0)"; exit 1; }
echo "f64_cmp_abs_mask ok"
