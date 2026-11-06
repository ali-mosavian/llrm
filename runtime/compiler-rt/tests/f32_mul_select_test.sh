#!/bin/sh
# usage: [LLRM_CC_DIR=dir with llrm-cc] tests/f32_mul_select_test.sh -> at m64 -O2 the f32 multiply holds no conditional jump between the imul and the pack call, and still equals the host FPU.
# Before SF_MUL_NORM_SELECT it branched on the product's top bit (47 or 46, half each on random operands): `jbe` after the imul, a mispredict every other call, mul 22 TSC against gcc's 8.
set -e
cd "$(dirname "$0")/.."; D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_CC_TARGET=x86_64-linux $D/llrm-cc -m64 -O2 -include config/x86_m64/config.h -Iinclude '-DSF_NAME(n)=sfb_##n' -c lib/fp/f32/mul.c -o $T/m.o
objdump -d --no-show-raw-insn $T/m.o | awk '/<sfb_mulsf3>:/{f=1;next} f&&/^$/{exit} f' > $T/body
grep -q imul $T/body || { echo "FAIL: no imul in sfb_mulsf3"; exit 1; }
n=$(awk '/imul/{s=1;next} s&&/call/{exit} s&&/\tj[a-z]+ /{c++} END{print c+0}' $T/body)
[ "$n" -eq 0 ] || { echo "FAIL: $n conditional jump(s) between the imul and the pack call"; exit 1; }
cc -O2 -fno-builtin -ffp-contract=off -DSF_MUL_NORM_SELECT -Iinclude -include stdint.h '-DSF_NAME(n)=sfb_##n' tests/host_f32.c $(tests/fp_sources.sh f32 f32_64) -lm -o $T/h
$T/h 2000000 | tail -1 | grep -qx 'fails=0' || { echo "FAIL: select form differs from the host FPU"; exit 1; }
echo "f32_mul_select ok"
