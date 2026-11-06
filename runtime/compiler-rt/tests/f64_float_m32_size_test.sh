#!/bin/bash
# usage: LLRM_CC_DIR=dir-with-llrm-cc LLRM_OW=dir tests/f64_float_m32_size_test.sh
# m32 and m64 f64 float.c at -O2 inlined the 64-bit normaliser into __floatdidf and __floatundidf: m32 668 B, m64 577 (compiler-rt 290).
# SF_CONV_MAG_NOINLINE (config/x86_m32) shares one copy: m32 345, m64 see PR. Bounds m32 380, m64 330.
cd "$(dirname "$0")/.." || exit 2
D=${LLRM_CC_DIR:?dir with llrm-cc}; T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT; bad=0
sz() { size -A "$1" | awk '$1==".text"||$1~/^\.rodata/{s+=$2}END{print s+0}'; }
LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h -O2 -c -Iinclude lib/fp/f64/float.c -o $T/a.o || exit 2
n=$(sz $T/a.o); [ "$n" -le 380 ] || { echo "FAIL: m32 float.c -O2 is $n B (bound 380)"; bad=1; }
LLRM_CC_TARGET=x86_64-linux $D/llrm-cc -DSF_INTABI=1 -include stdint.h -include config/x86_m64/config.h -O2 -c -Iinclude lib/fp/f64/float.c -o $T/b.o || exit 2
n=$(sz $T/b.o); [ "$n" -le 330 ] || { echo "FAIL: m64 float.c -O2 is $n B (bound 330)"; bad=1; }
[ $bad = 0 ] && echo "f64_float_m32_size ok"; exit $bad
