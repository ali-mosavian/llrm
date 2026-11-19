#!/bin/sh
# usage: LLRM_CC=<llrm-cc> tests/fix_nan_wasm_size_test.sh -> Code bytes of f64 fix_si.c and fix_usi.c at -Os for wasm32.
# config/wasm lacked SF_FIX_NAN_UNDEF: each carried a NaN test returning 0 that compiler-rt does not have (fix_si 151 B against 114, fix_usi 132 against 95).
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; bad=0
for spec in "fix_si|114" "fix_usi|95"; do
  f=${spec%%|*}; lim=${spec##*|}
  ${LLRM_CC:?llrm-cc} --target=wasm32 -Os -ffreestanding -include stdint.h -include config/wasm/config.h -Iinclude -c lib/fp/f64/$f.c -o $T/o.o || exit 2
  n=$(wasm-objdump -h $T/o.o | awk '$1=="Code"||$1=="Data"' | grep -o 'size=0x[0-9a-f]*' | cut -d= -f2 | while read h; do printf '%d\n' "$h"; done | paste -sd+ | bc)
  [ "$n" -le "$lim" ] || { echo "FAIL: wasm32 f64 $f.c is $n B (bound $lim)"; bad=1; }
done
[ $bad = 0 ] && echo "fix_nan_wasm_size ok"; exit $bad
