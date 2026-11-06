#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc tests/asm6502/i64div_cycles_test.sh
# 6502 -O2 __udivdi3/__umoddi3/__divdi3 ran the 64-bit pair loop for every operand: 8786 / 8786 / 8286 cycles per call on the 48 mixed pairs of bench_i64div.sh (llvm-mos 3970 / 3013 / 3853).
# SF_DIV64_NARROW (config/6502, -O2) divides one-word operands with the 32-bit divide and one-word divisors in 32 steps: 6042 / 6042 / 6576. Bounds 6500 / 6500 / 7000, no mismatches.
cd "$(dirname "$0")" || exit 2
out=$(sh ./bench_i64div.sh O2 2>&1) || { echo "$out"; exit 2; }; bad=0
chk() { n=$(echo "$out" | sed -n "s/^$1 -O2: ours cycles\/call=\([0-9]*\) .* fails=\([0-9]*\) |.*/\1 \2/p"); set -- $2 $n; [ -n "$2" ] && [ "$3" = 0 ] && [ "$2" -le "$1" ] || { echo "FAIL: $n (bound $1)"; bad=1; }; }
chk udiv 6500; chk umod 6500; chk sdiv 7000
[ $bad = 0 ] && echo "i64div_cycles ok"; exit $bad
