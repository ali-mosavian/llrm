#!/bin/bash
# usage: LLRM_CC_DIR=dir with llrm-cc tests/wasm_fix_size_test.sh
# wasm32 -O2 and -Os float-to-int objects with config/wasm: f64 fix_si 151 B, fix_usi 127, f32 126 / 109 (NaN tested for 0) against compiler-rt's 114 / 95 / 128 / 105.
# SF_FIX_NAN_UNDEF: 113 / 93 / 113 / 95. The bound is the compiler-rt object.
cd "$(dirname "$0")/.." || exit 2
D=${LLRM_CC_DIR:?dir with llrm-cc}; T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT; bad=0
for spec in f64/fix_si:114 f64/fix_usi:95 f32/fix_si:128 f32/fix_usi:105; do f=${spec%%:*}; lim=${spec##*:}
  for O in O2 Os; do
    LLRM_CC_TARGET=wasm32 $D/llrm-cc -include stdint.h -include config/wasm/config.h -$O -c -Iinclude lib/fp/$f.c -o $T/a.o || exit 2
    n=0; for h in $(wasm-objdump -h $T/a.o | awk '$1=="Code"||$1=="Data"' | grep -o 'size=0x[0-9a-f]*' | cut -d= -f2); do n=$((n + h)); done
    [ "$n" -le "$lim" ] || { echo "FAIL: wasm32 $f is $n B at -$O (compiler-rt $lim)"; bad=1; }
  done
done
[ $bad = 0 ] && echo "wasm_fix_size ok"; exit $bad
