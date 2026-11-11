#!/bin/sh
# usage: LLRM_CC=<llrm-cc> LLVM_MOS=<sdk> [MOS_RT=crates/target/llrm-6502/runtime/mos_rt.s] tests/asm6502/bench_f64_loop_step_test.sh
# bench_f64.py asserted that a 100-record run costs the sum of the single-record runs: it was 1683 clk (0.5%) under, the 17 clk by which stepping to the next record
# is cheaper than leaving the loop, 99 times. No f64 row could be read. The step is now measured (two pairs, which must agree) and added; the add row must print.
cd "$(dirname "$0")" || exit 2
: "${MOS_RT:=$(cd ../../../.. && pwd)/crates/target/llrm-6502/runtime/mos_rt.s}"; export MOS_RT
out=$(python3 bench_f64.py 20 add Os 2>&1) || { echo "FAIL: bench_f64.py: $(echo "$out" | tail -2)"; exit 1; }
echo "$out" | grep -q '^add |' || { echo "FAIL: no add row"; exit 1; }
echo "bench_f64_loop_step ok"
