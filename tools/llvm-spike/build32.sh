#!/bin/bash
# build32.sh PROG: llrm baseline and LLVM-mid-end builds (ELF, sysv, m32, -O2 and -Os) into $VSGCC_WORK
VT=${VT:-llvm}
set -e
P=$1; R=$HOME/scratch/llvm-spike/wt; O=$VSGCC_WORK; S=$R/bench/$P/$P.c
LLRM=$HOME/scratch/llvm-spike/target/release/llrm-c
EMU=$(python3 "$R/tools/linkrecipe.py" x86-m32 ld-emulation)
mkdir -p $O/b; cd $O/b
for o in O2 Os; do
  F="-m32 -mabi=sysv -$o -march=i486 -fno-inline-functions -fobject-format=elf"
  $LLRM $F -o $P.llrmElf$o.o $S 2>$P.llrmElf$o.err || echo "FAIL base$o $P"
  mkdir -p $O/d-${VT}/$P.$o
  LLRM_LLVM_MID=$o $LLRM $F --dump $O/d-${VT}/$P.$o -o $P.${VT}Elf$o.o $S 2>$P.${VT}Elf$o.err || echo "FAIL ${VT}$o $P: $(head -c 300 $P.${VT}Elf$o.err)"
  for c in llrmElf ${VT}Elf; do
    [ -f $P.$c$o.o ] && ld -m $EMU -static -e 0 -Ttext=0x10000 --just-symbols=$O/stub.elf -o $P.$c$o.elf $P.$c$o.o 2>$P.$c$o.lderr || echo "LINK FAIL $c$o $P"
  done
done
