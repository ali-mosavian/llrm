#!/bin/sh
# usage: LLRM_CC=<llrm-cc> LLVM_MOS=<sdk> tests/asm6502/mul32x32_default_test.sh
# f64 mul's 32x32 -> 64 is config/6502/mul32x32.s by default (hand asm, not reentrant): -Os 41312 clk per call as C, 11504 with it (llvm-mos 18270). The bound is 20000;
# SF_EXTRA= SF_ASM= (set, empty) builds the C and must fail it.
cd "$(dirname "$0")" || exit 2
: "${MOS_RT:=$(cd ../../../.. && pwd)/crates/target/llrm-6502/runtime/mos_rt.s}"; export MOS_RT
out=$(python3 bench_f64.py 20 mul Os 2>&1) || { echo "FAIL: bench_f64.py: $(echo "$out" | tail -2)"; exit 1; }
clk=$(echo "$out" | sed -n 's/^mul | \([0-9]*\) \/.*/\1/p')
[ "${clk:-999999}" -le 20000 ] || { echo "FAIL: f64 mul is ${clk:-?} clk per call (bound 20000)"; exit 1; }
echo "mul32x32_default ok"
