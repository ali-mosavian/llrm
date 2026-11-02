#!/bin/bash
# f64 executables for pricing: ours (sf64.c, sf32_conv64.c, int64.c, int32.c via llrm-cc, labels sf*/__* exported) and libgcc (lb1sf68.S _double/_floatex + fpgnulib), per CPU, ops 0..3.
# Needs /tmp/sfl from the f32 build (libgcc objects) and LLRM_CC_DIR. Writes ~/scratch/handoff/m68k-clocks/{ours,libgcc}64_<cpu>_Os_op<n>.
set -e
W=$(cd "$(dirname "$0")/.." && pwd); cd $W
export LD_LIBRARY_PATH=$HOME/scratch/gcc-m68k/root/usr/lib/x86_64-linux-gnu
B=$HOME/scratch/gcc-m68k/root/usr/bin/m68k-linux-gnu; D=${LLRM_CC_DIR:?set LLRM_CC_DIR to the dir holding llrm-cc}
O=$HOME/scratch/handoff/m68k-clocks; mkdir -p $O; T=$(mktemp -d)
cc -O2 -ffp-contract=off tests/gen_vectors64.c -o $T/gen && $T/gen 3000 > $T/vectors64.h
for c in 68000 68020; do
  for f in sf64 sf32_conv64 sf32_conv sf32 int64 int32; do
    LLRM_CC_TARGET=m68k-$c $D/llrm-cc -Os -S -Iinclude src/$f.c -o $T/$f.s
    awk 'BEGIN{print ".text"} /^(__|sf)[A-Za-z0-9_]*:$/ {l=$0; sub(/:$/,"",l); print ".globl " l} {print}' $T/$f.s > $T/$f.g.s
    $B-as -m$c -o $T/${f}_$c.o $T/$f.g.s
  done
  OBJS="$T/sf64_$c.o $T/sf32_conv64_$c.o $T/sf32_conv_$c.o $T/sf32_$c.o $T/int64_$c.o $T/int32_$c.o"
  INTS=; [ $c = 68000 ] && INTS="/tmp/sfl/${c}_divsi3.o /tmp/sfl/${c}_modsi3.o /tmp/sfl/${c}_udivsi3.o /tmp/sfl/${c}_umodsi3.o /tmp/sfl/${c}_mulsi3.o"
  for op in 0 1 2 3; do
    $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -DF64 -DOP=$op -I$T tests/m68k_clocks.c $OBJS -o $O/ours64_${c}_Os_op$op
    $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -DF64 -DOP=$op -I$T tests/m68k_clocks.c /tmp/sfl/${c}_double.o /tmp/sfl/${c}_floatex.o /tmp/sfl/${c}_fpg_Os.o $INTS -o $O/libgcc64_${c}_op$op
  done
done
cp $T/vectors64.h $O/; ls $O | wc -l
