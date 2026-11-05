#!/bin/sh
# usage: [LLRM_CC_DIR=dir of llrm-cc] tests/wasm_clz.sh -> with config/wasm/config.h the soft-float sources lower sf_clz32 to i32.clz.
# Without the config (none existed) the wasm output of float.c had no clz op: every use was the generic 5-step ladder.
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:-$HOME/scratch/co1-dist-base-t/dist}; T=$(mktemp -d); trap 'rm -r $T' EXIT
for f in f32/float_si f64/float; do
  LLRM_CC_TARGET=wasm32 $D/llrm-cc -Os -c -DSF_INTABI -include config/wasm/config.h -Iinclude lib/fp/$f.c -o $T/a.o || exit 2
  wasm2wat $T/a.o | grep -q 'i32.clz' || { echo "FAIL: $f has no i32.clz with config/wasm/config.h"; exit 1; }
done
echo "wasm_clz ok"
