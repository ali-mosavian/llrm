#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc tests/asm6502/i64div_cycles_test.sh
# 6502 -O2 __udivdi3/__umoddi3/__divdi3 ran the 64-bit pair loop for every operand: 8786 / 8786 / 8286 cycles per call on the 48 mixed pairs of bench_i64div.sh (llvm-mos 3970 / 3013 / 3853).
# SF_DIV64_NARROW (config/6502, -O2) runs one-word divisors as a one-word remainder over the dividend bits: 6424 / 6424 / 7272. Bounds 6700 / 6700 / 7600, no mismatches.
cd "$(dirname "$0")" || exit 2
out=$(sh ./bench_i64div.sh O2 2>&1) || { echo "$out"; exit 2; }; bad=0
chk() { n=$(echo "$out" | sed -n "s/^$1 -O2: ours cycles\/call=\([0-9]*\) .* fails=\([0-9]*\) |.*/\1 \2/p"); set -- $2 $n; [ -n "$2" ] && [ "$3" = 0 ] && [ "$2" -le "$1" ] || { echo "FAIL: $n (bound $1)"; bad=1; }; }
chk udiv 6700; chk umod 6700; chk sdiv 7600
[ $bad = 0 ] && echo "i64div_cycles ok"; exit $bad
