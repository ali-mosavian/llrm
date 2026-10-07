#!/bin/bash
# run32.sh: every program x variant through the harness, JSON lines to $VSGCC_WORK/results.jsonl
R=$HOME/scratch/llvm-spike/wt; H=$R/crates/target/llrm-x86-m32/vsgcc
cd $R
for p in $(ls bench | grep -v -E "readme|parity|huge|textfill|grep"); do for v in llrmElfO2 llrmElfOs ${VTS:-llvmElfO2 llvmElfOs}; do
  [ -f $VSGCC_WORK/b/$p.$v.elf ] && echo "$p $v"; done; done | xargs -P 16 -L1 bash -c 'uv run --project tools python '$H'/harness.py $0 $1 2>&1 | tail -1' >> $VSGCC_WORK/results.jsonl
