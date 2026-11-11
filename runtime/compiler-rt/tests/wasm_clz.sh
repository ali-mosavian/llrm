#!/bin/sh
# usage: [LLRM_CC_DIR=dir of llrm-cc] tests/wasm_clz.sh -> with config/wasm/config.h the soft-float sources lower sf_clz32/sf_clz64 to i32.clz/i64.clz.
# Without the config (none existed) the wasm output of float.c had no clz op: every use was the generic ladder.
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:-$HOME/scratch/co1-dist-base-t/dist}; T=$(mktemp -d); trap 'rm -r $T' EXIT
for c in f32/float_si:i32.clz f64/float:i64.clz; do
  f=${c%:*}; op=${c#*:}
  LLRM_CC_TARGET=wasm32 $D/llrm-cc -Os -c -DSF_INTABI -include config/wasm/config.h -Iinclude lib/fp/$f.c -o $T/a.o || exit 2
  wasm2wat $T/a.o | grep -q "$op" || { echo "FAIL: $f has no $op with config/wasm/config.h"; exit 1; }
done
echo "wasm_clz ok"
