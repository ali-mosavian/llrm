#!/bin/bash
# run.sh: build llrm-c (release), every program with every compiler, run them all, print the tables.
# Needs gcc, clang (32-bit objects), GNU ld/nm/size, uv. Products go to $VSGCC_WORK (default ~/scratch/vsgcc-work).
set -e
R=$(cd "$(dirname "$0")/../../../.." && pwd); H=$R/crates/target/llrm-x86-code32/vsgcc; export VSGCC_WORK=${VSGCC_WORK:-$HOME/scratch/vsgcc-work}
PROGS=$(ls $R/bench | grep -v -E "readme|parity|huge|textfill|grep")   # 16-bit only, no input, or timed only
mkdir -p $VSGCC_WORK
(cd $R && cargo build --release --bin llrm-c -q)
gcc -m32 -c $H/stub.s -o $VSGCC_WORK/stub.o && ld -m elf_i386 -static -e 0 -Ttext=0x8000 -o $VSGCC_WORK/stub.elf $VSGCC_WORK/stub.o
for p in $PROGS; do $H/build.sh $p; done
: > $VSGCC_WORK/results.jsonl
for p in $PROGS; do for v in llrm llrmOs gccO2 gccOs clangO2 clangOs; do
  uv run --project $R/tools python $H/harness.py $p $v | tail -1 >> $VSGCC_WORK/results.jsonl
done; done
uv run --project $R/tools python $H/table.py
for p in $PROGS; do uv run --project $R/tools python $H/loops.py $p llrm gccO2 clangO2 > $VSGCC_WORK/loops.$p.txt; done
uv run --project $R/tools python $H/ctime.py   # idle machine: it times
