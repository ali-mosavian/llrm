#!/bin/bash
# usage: tests/f32_asm_snapshot.sh OUTDIR -> -S output of sf32.c and sf32_conv.c for every compiler/target/config the f32 numbers come from.
# Diff two snapshots (before/after a refactor): identical files mean no f32 cell can have moved. Needs the env of size_m16.sh and size_m68k.sh.
cd "$(dirname "$0")/.."; O=$1; mkdir -p $O
RB=${SNAP_LLRM:-$HOME/scratch/sf-cand-target/release}; M68=${SNAP_M68K:-$HOME/scratch/coord-ps-target/release}; B6=${SNAP_6502:-$HOME/scratch/sf-bin6502}
for f in sf32 sf32_conv; do for o in Os O2; do
  LLRM_CC_TARGET=x86_16-wat $RB/llrm-cc -march=i486 -$o -S -Iinclude -include include/sf_x86_m16.h src/$f.c -o $O/m16.$f.$o.s 2>$O/m16.$f.$o.err || echo "FAIL m16 $f $o"
  LLRM_CC_TARGET=x86_32-linux $RB/llrm-cc -march=i486 -$o -S -Iinclude -DSF_INTABI=1 -include include/sf_x86_m32.h src/$f.c -o $O/m32.$f.$o.s || echo "FAIL m32 $f $o"
  for c in 68000 68020; do LLRM_CC_TARGET=m68k-$c $M68/llrm-cc -$o -S -Iinclude -DSF_INTABI=1 -include include/sf_m68k.h src/$f.c -o $O/m68k$c.$f.$o.s || echo "FAIL m68k $c $f $o"; done
  $B6/llrm-cc --target=6502-sim65 -$o -S -Iinclude -DSF_INTABI src/$f.c -o $O/6502.$f.$o.s || echo "FAIL 6502 $f $o"
  for cc in gcc clang; do $cc -m32 -$o -S -ffreestanding -fno-pic -fno-asynchronous-unwind-tables -w -Iinclude -DSF_INTABI=1 -include stdint.h -include include/sf_x86_m32.h src/$f.c -o $O/$cc.$f.$o.s || echo "FAIL $cc $f $o"; done
done; done
ls $O/*.s | wc -l
