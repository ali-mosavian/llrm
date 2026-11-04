#!/bin/bash
# f64 mul folded the product with variable 64-bit shifts (sh = 49 or 50): at x86 m32 -Os mul.o was 679 B and the m32 -O2 mul 191653 clk per 400 calls.
# Constant shifts per arm: 635 B, 172217 clk. Fails above 650 B.
cd "$(dirname "$0")/.."; . tests/obj_bytes.sh
D=${LLRM_CC_DIR:-$HOME/targets/gs-out/rbase}; T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -Os -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h -Iinclude -c lib/fp/f64/mul.c -o $T/m.o || exit 2
b=$(obj_bytes $T/m.o)
[ "$b" -le 650 ] || { echo "FAIL: f64 mul.o is $b B at m32 -Os (limit 650; variable shifts gave 679)"; exit 1; }
echo "f64 mul ok: $b B"
