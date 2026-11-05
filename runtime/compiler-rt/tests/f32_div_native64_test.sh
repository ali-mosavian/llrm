#!/bin/sh
# usage: LLRM_CC_DIR=dir tests/f32_div_native64_test.sh -> at m64 -O2 the f32 divide is one hardware divide (it was four 32-bit divides plus the remainder steps, SF_DIV_HW32) and still equals the host FPU.
set -e
cd "$(dirname "$0")/.."; D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_CC_TARGET=x86_64-linux $D/llrm-cc -m64 -O2 -include config/x86_m64/config.h -Iinclude '-DSF_NAME(n)=sfb_##n' -c lib/fp/f32/div.c -o $T/d.o
n=$(objdump -d --no-show-raw-insn $T/d.o | awk '/<sfb_divsf3>:/{f=1;next} f&&/^$/{exit} f' | grep -cE '\sdiv[lq]?\s')
[ "$n" -eq 1 ] || { echo "FAIL: sfb_divsf3 has $n div instructions, want 1"; exit 1; }
cc -O2 -fno-builtin -ffp-contract=off -DSF_DIV_NATIVE64 -Iinclude -include stdint.h '-DSF_NAME(n)=sfb_##n' tests/host_f32.c $(tests/fp_sources.sh f32 f32_64) -lm -o $T/h
$T/h 2000000 | tail -1 | grep -qx 'fails=0' || { echo "FAIL: native64 divide differs from the host FPU"; exit 1; }
LLRM_CC_TARGET=x86_64-linux $D/llrm-cc -m64 -O2 -include config/x86_m64/config.h -Iinclude '-DSF_NAME(n)=sfb_##n' -c lib/fp/f32/pack.c -o $T/p.o
gcc -O2 -w tests/f32_native_div.c $T/d.o $T/p.o -o $T/nat
$T/nat || { echo "FAIL: the llrm-cc-built divide differs from SSE (repro: tests/f32_native_div.c; (u64)a << 27 is emitted as a 32-bit shl before div rcx)"; exit 1; }
echo "f32_div_native64 ok"
