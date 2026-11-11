#!/bin/bash
# wasm32 -O2 __divdf3 executed 1313 ops/call (compiler-rt 256): the restoring bit loop. SF_DIV_NEWTON in config/wasm brings it to ~390; bound 500.
H=$(cd "$(dirname "$0")" && pwd)
out=$(OPS=div bash "$H/../../../tools/softfloat_cells/wasm/wasm_icount.sh" "$H/.." 2>&1)
n=$(sed -n 's/.*div: ours \([0-9.]*\) crt.*/\1/p' <<< "$out")
[ -n "$n" ] || { echo "FAIL: no div line: $out"; exit 1; }
awk -v v="$n" 'BEGIN{exit !(v<500)}' || { echo "FAIL: wasm32 -O2 divdf3 $n ops/call, bound 500"; exit 1; }
echo "ok: wasm32 -O2 divdf3 $n ops/call"
