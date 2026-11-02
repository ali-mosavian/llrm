#!/bin/sh
# usage: tests/z80/run.sh [ops]   (mul udiv umod div mod fadd fsub fmul fdiv citof cutof cftoi cftou)
# Runs src/z80/int32.s on ucsim_z80 against the host: mismatches and mean cycles per call (call minus a bare ret).
set -e
cd "$(dirname "$0")"; OPS=${*:-mul udiv umod div mod}; T=$(mktemp -d); S=../../src/z80
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > $T/crt0.s
sdasz80 -o $T/crt0.rel $T/crt0.s
for f in $S/*.s; do b=$(basename $f .s); cp $f $T/$b.s; sdasz80 -g -o $T/$b.rel $T/$b.s; done
for op in $OPS; do
  case $op in mul) f=__mulsi3;; udiv) f=__udivsi3;; umod) f=__umodsi3;; div) f=__divsi3;; mod) f=__modsi3;; fadd) f=__addsf3;; fsub) f=__subsf3;; fmul) f=__mulsf3;; fdiv) f=__divsf3;; citof) f=__floatsisf;; cutof) f=__floatunsisf;; cftoi) f=__fixsfsi;; cftou) f=__fixunssfsi;; esac
  case $op in f*|c*) G=gen_f32.py; A=${op#?};; *) G=gen.py; A=$op;; esac
  python3 $G $A > $T/vec.inc
  sed "s/OP/$f/g" drv.s > $T/drv.s
  printf '\t.module stub\n\t.area _CODE\n\t.globl %s\n%s:\n\tret\n' $f $f > $T/stub.s
  (cd $T; sdasz80 -o drv.rel drv.s; sdasz80 -o stub.rel stub.s
   sdldz80 -i -b _CODE=0x100 s.ihx crt0.rel drv.rel stub.rel >/dev/null
   sdldz80 -i -b _CODE=0x100 r.ihx crt0.rel drv.rel int32.rel sf32_cmp.rel sf32_arith.rel >/dev/null)
  run() { printf 'run\nstate\ndump rom 0x8f00 0x8f01\ndump rom 0x8f10 0x8f1f\nquit\n' | ucsim_z80 -t Z80 -q -w $T/$1.ihx -c - 2>&1; }
  b=$(run s | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
  o=$(run r); c=$(echo "$o" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
  f2=$(echo "$o" | sed -n 's/^0x8f00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
  [ "${f2:-ffff}" != 0000 ] && echo "$o" | grep -E "^0x8f1" | head -3
  nv=$(sed -n 's/^NV = //p' $T/vec.inc)
  echo "$op fails=$((0x${f2:-ffff})) cycles=$(( (c-b)/nv ))"
done
rm -r $T
