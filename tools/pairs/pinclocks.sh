#!/bin/bash
# pinclocks.sh LABEL "EXTRA clang flags": the rows programs built by the PINNED clang (m32 i486) at -O2 and -O3 with the flags (e.g. "-mllvm -enable-misched=false"),
# run in the 486 clock model (crates/target/*/vsgcc/harness.py): JSON lines in $PINRUN_WORK/LABEL/res.jsonl with clocks, ins, mem, code, and the static Spill/Reload/register-copy
# counts of its -S. Needs a dist dir with the stub tools in LLRM_BIN (any stamped dist) and the rows corpus ROWS_MAIN24D/b (default ~/scratch/rows/main24d).
set -u
L=$1; X=$2
R=$(cd "$(dirname "$0")/../.." && pwd)
H=$(echo $R/crates/target/*/vsgcc)
W=${PINRUN_WORK:-$HOME/scratch/pinrun}/$L; SRC=${ROWS_MAIN24D:-$HOME/scratch/rows/main24d}
CLANG=${LLVM_BIN:-$HOME/scratch/llvm-build/bin}/clang
mkdir -p $W/b; cp $SRC/abi $W/ 2>/dev/null; VSGCC_WORK=$W; eval "$(grep -E '^(gcc -m32 -c|sha256sum) ' $H/run.sh)"; unset VSGCC_WORK; cp $SRC/b/*.c $W/b/ 2>/dev/null; rm -f $W/res.jsonl
EMU=$(python3 $R/tools/linkrecipe.py x86-m32 ld-emulation)
LIBGCC=$(gcc -m32 -print-libgcc-file-name)
F="-Dfar= -m32 -march=i486 -fno-pic -fno-stack-protector -fcf-protection=none -fno-asynchronous-unwind-tables"
one(){ P=$1; o=$2
  cd $W/b
  $CLANG $F -mstack-alignment=4 -$o $X -c -o $P.clang$o.o $P.c 2>$P.clang$o.err || { echo "FAIL $P $o" >> $W/fails; return; }
  $CLANG $F -mstack-alignment=4 -$o $X -S -o $P.clang$o.s $P.c 2>/dev/null
  ld -m $EMU -static -e 0 -Ttext=0x10000 --just-symbols=$W/stub.elf -Map=$P.clang$o.map -o $P.clang$o.elf $P.clang$o.o $LIBGCC 2>$P.clang$o.lderr || { echo "LINK $P $o" >> $W/fails; return; }
  j=$(VSGCC_WORK=$W uv run -q --project $R/tools python $H/harness.py $P clang$o 2>/dev/null | tail -1)
  sp=$(grep -c "Spill" $P.clang$o.s); rl=$(grep -c "Reload" $P.clang$o.s); cp=$(grep -cE "^\s+mov[lwb]\s+%[a-z]+, %[a-z]+$" $P.clang$o.s)
  echo "{\"prog\":\"$P\",\"o\":\"$o\",\"spill\":$sp,\"reload\":$rl,\"copy\":$cp,\"row\":${j:-null}}" >> $W/res.jsonl
}
export -f one; export W X CLANG F EMU LIBGCC R H
PROGS=$(ls $SRC/b/*.c | xargs -n1 basename | sed 's/\.c$//' | grep -v '\.' | sort)
for o in O2 O3; do for P in $PROGS; do echo "$P $o"; done; done | xargs -P 12 -L1 bash -c 'one $0 $1'
echo "$L done: $(wc -l < $W/res.jsonl) rows, fails: $(cat $W/fails 2>/dev/null | wc -l)"
