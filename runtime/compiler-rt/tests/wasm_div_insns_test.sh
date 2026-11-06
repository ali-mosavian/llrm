#!/bin/bash
# f64 div on wasm32 -O2 divided one quotient bit per step: 1313 executed ops per call against compiler-rt's 256. config/wasm now picks SF_DIV64_UDIV (two i64.div_u digits): 268. Bound 400.
cd "$(dirname "$0")/.." || exit 2
n=$(bash ../../tools/softfloat_cells/wasm/wasm_icount.sh 2>&1 | sed -n 's/^wasm32 -O2 div: ours \([0-9]*\).*/\1/p')
[ -n "$n" ] || { echo "FAIL: no div reading from wasm_icount.sh"; exit 2; }
[ "$n" -le 400 ] && [ "$n" -gt 50 ] || { echo "FAIL: wasm32 f64 div executes $n ops per call (bound 400, floor 50)"; exit 1; }
echo "wasm_div_insns ok ($n)"
