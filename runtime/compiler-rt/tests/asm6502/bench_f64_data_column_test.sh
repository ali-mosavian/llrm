#!/bin/sh
# usage: LLRM_CC=<llrm-cc> LLVM_MOS=<sdk> [MOS_RT=...] tests/asm6502/bench_f64_data_column_test.sh
# bench_f64.py printed the code bytes only: the library's zero-filled static cells (f64 mul: 43 B of BSS at -O2) were in no column, so a bar read as code+data was short
# by them. Each ours cell now names `code N B, data M B`; the mul row at -O2 must carry data above zero.
cd "$(dirname "$0")" || exit 2
: "${MOS_RT:=$(cd ../../../.. && pwd)/crates/target/llrm-6502/runtime/mos_rt.s}"; export MOS_RT
out=$(python3 bench_f64.py 20 mul O2 2>&1) || { echo "FAIL: bench_f64.py: $(echo "$out" | tail -2)"; exit 1; }
d=$(echo "$out" | sed -n 's/^mul |.*, data \([0-9]*\) B.*/\1/p' | head -1)
[ -n "$d" ] && [ "$d" -gt 0 ] || { echo "FAIL: the mul -O2 data column is '${d:-missing}', not above zero"; echo "$out" | tail -3; exit 1; }
echo "bench_f64_data_column ok (data $d B)"
