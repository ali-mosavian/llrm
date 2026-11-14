#!/bin/bash
# usage: [LLRM_CC_DIR=dir with llrm-cc] tests/fix_si_size_test.sh
# f32 fix_si.c shifted the significand left or right by e-23: m32 -O2 107 B (spilled a register across the shift), -Os 94 (compiler-rt 92).
# One right shift of the hidden-bit-at-31 significand: m32 70 / 67, m64 62 / 63, wasm 106. Bounds m32 75, m64 66, wasm 110. Outputs: fixsfsi_all.c.
cd "$(dirname "$0")/.." || exit 2
D=${LLRM_CC_DIR:-$HOME/scratch/sf-fp128-target/debug}; T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT; bad=0
chk() { [ "$2" -le "$3" ] || { echo "FAIL: $1 fix_si.c is $2 B (bound $3)"; bad=1; }; }
sz() { size -A "$1" | awk '$1~/^\.text/||$1~/^\.rodata/{s+=$2}END{print s+0}'; }
for O in O2 Os; do
  LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h -$O -c -Iinclude lib/fp/f32/fix_si.c -o $T/a.o || exit 2
  chk "m32 -$O" "$(sz $T/a.o)" 75
  LLRM_CC_TARGET=x86_64-linux $D/llrm-cc -DSF_INTABI=1 -include stdint.h -include config/x86_m64/config.h -$O -c -Iinclude lib/fp/f32/fix_si.c -o $T/b.o || exit 2
  chk "m64 -$O" "$(sz $T/b.o)" 66
  LLRM_CC_TARGET=wasm32 $D/llrm-cc -include stdint.h -include config/wasm/config.h -$O -c -Iinclude lib/fp/f32/fix_si.c -o $T/c.o || exit 2
  n=0; for h in $(wasm-objdump -h $T/c.o | awk '$1=="Code"||$1=="Data"' | grep -o 'size=0x[0-9a-f]*' | cut -d= -f2); do n=$((n + h)); done
  chk "wasm32 -$O" "$n" 110
done
cc -O2 -Iinclude tests/fixsfsi_all.c lib/fp/f32/fix_si.c lib/fp/f32/fix_usi.c lib/fp/f32/float_si.c -o $T/all || exit 2
$T/all || bad=1
[ $bad = 0 ] && echo "fix_si_size ok"; exit $bad
