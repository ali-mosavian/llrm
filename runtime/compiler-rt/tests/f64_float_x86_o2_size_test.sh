#!/bin/bash
# usage: LLRM_CC_DIR=dir with llrm-cc tests/f64_float_x86_o2_size_test.sh
# m32 -O2 f64 float.c (four conversions) was 668 B: sidf/unsidf went through pack only at -Os. DIRECT32 at every level: 345 B (compiler-rt's four: 290). Bound 380.
cd "$(dirname "$0")/.." || exit 2
D=${LLRM_CC_DIR:?dir with llrm-cc}; T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h -O2 -c -Iinclude lib/fp/f64/float.c -o $T/a.o || exit 2
n=$(size -A $T/a.o | awk '$1==".text"||$1~/^\.rodata/{s+=$2}END{print s}')
[ "$n" -le 380 ] || { echo "FAIL: m32 f64 float.c is $n B at -O2 (bound 380)"; exit 1; }
echo "f64_float_x86_o2_size ok ($n)"
