#!/bin/bash
# run.sh: build llrm-c (release), every program with every compiler, run them all, print the tables.
# Needs gcc, clang (32-bit objects), GNU ld/nm/size, uv. Products go to $VSGCC_WORK (default ~/scratch/vsgcc-work).
set -eo pipefail   # a harness that fails (an unresolved symbol) stops the run, not a blank row
R=$(cd "$(dirname "$0")/../../../.." && pwd); H=$R/crates/target/llrm-x86-m32/vsgcc; export VSGCC_WORK=${VSGCC_WORK:-$HOME/scratch/vsgcc-work}
PROGS=$(ls $R/bench | grep -v -E "readme|parity|huge|textfill|grep")   # 16-bit only, no input, or timed only
PROGS="$PROGS $(ls $H/kernels)"                                          # the x_ kernels
mkdir -p $VSGCC_WORK
(cd $R && cargo build --release --bin llrm-c -q)
gcc -m32 -c $H/stub.s -o $VSGCC_WORK/stub.o && ld -m $(python3 $R/tools/linkrecipe.py x86-m32 ld-emulation) -static -e 0 -Ttext=0x8000 -o $VSGCC_WORK/stub.elf $VSGCC_WORK/stub.o
sha256sum $H/stub.s | cut -d" " -f1 | tr -d "\n" > $VSGCC_WORK/stub.elf.src
for p in $PROGS; do $H/build.sh $p; done
: > $VSGCC_WORK/results.jsonl
for p in $PROGS; do for v in llrm llrmOs gccO2 gccOs clangO2 clangOs; do
  uv run --project $R/tools python $H/harness.py $p $v | tail -1 >> $VSGCC_WORK/results.jsonl
done; done
uv run --project $R/tools python $H/table.py
for p in $PROGS; do uv run --project $R/tools python $H/loops.py $p llrm gccO2 clangO2 > $VSGCC_WORK/loops.$p.txt; done
uv run --project $R/tools python $H/ctime.py   # idle machine: it times
