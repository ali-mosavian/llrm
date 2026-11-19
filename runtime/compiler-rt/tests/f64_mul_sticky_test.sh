#!/bin/bash
# f64 mul tested the dropped product bits as `(lo << 14) != 0`: a 64-bit shift is shld+shl on a 32-bit x86. With SF_STICKY_MASK (config/x86_m32) it is a mask test:
# shld 9 -> 8 in mul.c at -O2 and, over 400 muls, m32 -O2 166617 -> 157017 clk (-5.8%), -Os image 1100 -> 1052 B. Fails above 8 shld.
cd "$(dirname "$0")/.."
D=${LLRM_CC_DIR:-$HOME/scratch/x86m32-live-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -O2 -S -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h -Iinclude lib/fp/f64/mul.c -o $T/m.s || exit 2
n=$(grep -c -i shld $T/m.s)
[ "$n" -le 8 ] || { echo "FAIL: f64 mul has $n shld at m32 -O2 (limit 8)"; exit 1; }
echo "f64_mul_sticky ok ($n shld)"
