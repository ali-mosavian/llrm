#!/bin/bash
# usage: LLRM_CC_DIR=dir-with-llrm-cc [CLANG_WASM=pin clang with wasm] tests/wasm_sidf_size_test.sh
# SF-WASM-SIDF: f64 __floatsidf / __floatunsidf linked alone (wasm-ld --gc-sections, one export, whole file). From the shared from_mag32 (SF_CONV_NOINLINE): sidf 366 B,
# unsidf 346 (from_mag32 41 B + entry 37 / 13); the pin's compiler-rt (clang 24, wasm) 484 / 456. The earlier "+15" (475 vs 460) compared against system clang 20's compiler-rt, 107 B smaller than the pin's.
# Bounds: from_mag32 45, sidf 380, unsidf 360, and ours <= the pin's compiler-rt when CLANG_WASM exists.
cd "$(dirname "$0")/.." || exit 2
D=${LLRM_CC_DIR:?dir with llrm-cc}; CRT=${CRT:-$HOME/scratch/llvm-src/compiler-rt/lib/builtins}; CW=${CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang}
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT; bad=0; CFG=${SF_WASM_CONFIG:-config/wasm/config.h}
chk() { [ "$2" -le "$3" ] || { echo "FAIL: $1 is $2 B (bound $3)"; bad=1; }; }
for O in O2 Os; do
  LLRM_CC_TARGET=wasm32 $D/llrm-cc -include stdint.h -include $CFG -$O -c -Iinclude lib/fp/f64/float.c -o $T/o.o || exit 2
  for spec in __floatsidf:380 __floatunsidf:360; do e=${spec%:*}; lim=${spec#*:}
    wasm-ld --no-entry --gc-sections --export=$e -o $T/o.wasm $T/o.o || exit 2
    n=$(stat -c %s $T/o.wasm); chk "$e -$O linked alone" $n $lim
    m=$(wasm-objdump -x $T/o.wasm | sed -n 's/.*size=\([0-9]*\) <from_mag32>.*/\1/p'); chk "from_mag32 -$O" "${m:-9999}" 45
    if [ -x "$CW" ]; then
      $CW --target=wasm32 -$O -w -ffreestanding -I$CRT -c $CRT/${e#__}.c -o $T/c.o && wasm-ld --no-entry --gc-sections --export=$e -o $T/c.wasm $T/c.o && chk "$e -$O vs pin compiler-rt" $n $(stat -c %s $T/c.wasm)
    fi
  done
done
[ $bad = 0 ] && echo "wasm_sidf_size ok"; exit $bad
