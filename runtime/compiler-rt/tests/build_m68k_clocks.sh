#!/bin/sh
# builds ours and libgcc exes per CPU and op into ~/scratch/handoff/m68k-clocks LLRM_CC_DIR is required.
set -e
W=$(cd "$(dirname "$0")/.." && pwd); cd $W
export LD_LIBRARY_PATH=$HOME/scratch/gcc-m68k/root/usr/lib/x86_64-linux-gnu
B=$HOME/scratch/gcc-m68k/root/usr/bin/m68k-linux-gnu; D=${LLRM_CC_DIR:?set LLRM_CC_DIR to the dir holding llrm-cc}
O=$HOME/scratch/handoff/m68k-clocks; mkdir -p $O; T=$(mktemp -d)
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen 3000 > $T/vectors.h
for c in 68000 68020; do for opt in Os O2; do
  OBJS=; for src in $(tests/fp_sources.sh f32 int | grep -v int64); do f=$(basename $src .c)
    LLRM_CC_TARGET=m68k-$c $D/llrm-cc -$opt -S -Iinclude $src -o $T/$f.s
    awk 'BEGIN{print ".text"} /^[A-Za-z_][A-Za-z0-9_]*:$/ {l=$0; sub(/:$/,"",l); print ".globl " l} {print}' $T/$f.s > $T/$f.g.s
    $B-as -m$c -o $T/${f}_$c$opt.o $T/$f.g.s; OBJS="$OBJS $T/${f}_$c$opt.o"
  done
  for op in 0 1 2 3 4; do
    $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -DOP=$op -I$T tests/m68k_clocks.c $OBJS -o $O/ours_${c}_${opt}_op$op
  done
done
  INTS=; [ $c = 68000 ] && INTS="/tmp/sfl/${c}_divsi3.o /tmp/sfl/${c}_modsi3.o /tmp/sfl/${c}_udivsi3.o /tmp/sfl/${c}_umodsi3.o /tmp/sfl/${c}_mulsi3.o"
  for op in 0 1 2 3 4; do
    $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -DOP=$op -I$T tests/m68k_clocks.c /tmp/sfl/${c}_float.o /tmp/sfl/${c}_{eq,ne,gt,ge,lt,le}sf2.o /tmp/sfl/${c}_fpg_Os.o /tmp/sfl/${c}_double.o /tmp/sfl/${c}_floatex.o /tmp/sfl/${c}_fus.o $INTS -o $O/libgcc_${c}_op$op 2>&1 | head -3
  done
done
cp $T/vectors.h $O/vectors.h; ls $O | wc -l
