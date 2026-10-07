#!/bin/bash
# gate16.sh TAG [ENV=VAL...]: the m16 gate's 230 measurements with the LLVM mid end under env (O2 and Os in parallel)
T=$1; shift; for kv in "$@"; do export "$kv"; done
export LLRM_BENCH_COMPILE_TIMEOUT=60 CARGO_TARGET_DIR=$HOME/scratch/llvm-spike/target; G=$HOME/scratch/llvm-spike/gate
cd $HOME/scratch/llvm-spike/wt
for o in O2 Os; do rm -rf $G/work-$T-$o
  LLRM_LLVM_MID=$o LLRM_LLVM_TRIPLE=${LLRM_LLVM_TRIPLE:-i386-unknown-linux-gnu} LLRM_LLVM_CPU=${LLRM_LLVM_CPU:-i386} uv run --project tools python tools/bench/bench.py --opt $o --json $G/m16-$T-$o.json --work $G/work-$T-$o > $G/m16-$T-$o.txt 2>&1 &
done
wait
