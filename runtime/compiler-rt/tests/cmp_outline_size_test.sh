#!/bin/bash
# usage: [LLRM_CC_DIR=dir with llrm-cc] tests/cmp_outline_size_test.sh
# m32 and wasm32 -O2 inlined the compare core into each of the seven entries: cmp.c f64 1026 / 784 B, f32 662 / 700 B (compiler-rt: 399 / 337, 268 / 267).
# With SF_CMP_OUTLINE the entries tail-jump to one cmp core and gt to ge: see the PR for the figures. Bounds m32 420 / 230, wasm 250 / 215 (entries are aliases there: f32 275 -> 210).
cd "$(dirname "$0")/.." || exit 2
D=${LLRM_CC_DIR:?dir with llrm-cc}; T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT; bad=0
for w in f64 f32; do
  LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h -O2 -c -Iinclude lib/fp/$w/cmp.c -o $T/a.o || exit 2
  n=$(size -A $T/a.o | awk '$1~/^\.text/||$1~/^\.rodata/{s+=$2}END{print s}'); lim=420; [ $w = f32 ] && lim=230
  [ "$n" -le $lim ] || { echo "FAIL: m32 $w cmp.c is $n B at -O2 (bound $lim)"; bad=1; }
  LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h -Os -c -Iinclude lib/fp/$w/cmp.c -o $T/s.o || exit 2
  n=$(size -A $T/s.o | awk '$1~/^\.text/||$1~/^\.rodata/{s+=$2}END{print s}'); lim=175; [ $w = f32 ] && lim=100 # eq/ne/lt/le aliases: 17 B wrappers were f32 120 / f64 193
  [ "$n" -le $lim ] || { echo "FAIL: m32 $w cmp.c is $n B at -Os (bound $lim)"; bad=1; }
  LLRM_CC_TARGET=x86_64-linux $D/llrm-cc -DSF_INTABI=1 -include stdint.h -include config/x86_m64/config.h -O2 -c -Iinclude lib/fp/$w/cmp.c -o $T/c.o || exit 2
  n=$(size -A $T/c.o | awk '$1~/^\.text/||$1~/^\.rodata/{s+=$2}END{print s}'); lim=300; [ $w = f32 ] && lim=230
  [ "$n" -le $lim ] || { echo "FAIL: m64 $w cmp.c is $n B at -O2 (bound $lim)"; bad=1; }
  LLRM_CC_TARGET=wasm32 $D/llrm-cc -include stdint.h -include config/wasm/config.h -O2 -c -Iinclude lib/fp/$w/cmp.c -o $T/b.o || exit 2
  n=0; for h in $(wasm-objdump -h $T/b.o | awk '$1=="Code"||$1=="Data"' | grep -o 'size=0x[0-9a-f]*' | cut -d= -f2); do n=$((n + h)); done; lim=250; [ $w = f32 ] && lim=215
  [ "$n" -le $lim ] || { echo "FAIL: wasm32 $w cmp.c is $n B at -O2 (bound $lim)"; bad=1; }
done
[ $bad = 0 ] && echo "cmp_outline_size ok"; exit $bad
