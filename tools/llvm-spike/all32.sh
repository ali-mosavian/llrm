#!/bin/bash
# all32.sh TAG [ENV=VAL...]: build every program with the LLVM mid end under env, as variant TAGElf, run the harness for it
export VSGCC_WORK=${VSGCC_WORK:-$HOME/scratch/llvm-spike/work}
T=$1; shift; export VT=$T; for kv in "$@"; do export "$kv"; done
cd $HOME/scratch/llvm-spike/wt
ls bench | grep -v -E "readme|parity|huge|textfill|grep" | xargs -P 16 -I{} $HOME/scratch/llvm-spike/bin/build32.sh {} 2>&1 | grep -v "^$" | tail -5
VTS="${T}ElfO2 ${T}ElfOs" $HOME/scratch/llvm-spike/bin/run32.sh
