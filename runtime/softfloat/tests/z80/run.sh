#!/bin/sh
# usage: tests/z80/run.sh [ops]   (mul udiv umod div mod fadd fsub fmul fdiv citof cutof cftoi cftou)
# EDGE=1: gen_f32.py's edge records. SF_DEFS='SF_MUL_LUT ...' assembles the sources with those symbols defined.
# Runs src/z80/int32.s on ucsim_z80 against the host: mismatches and mean cycles per call (call minus a bare ret; the bare-ret run passes its records, basevec.py).
set -e
cd "$(dirname "$0")"; OPS=${*:-mul udiv umod div mod}; T=$(mktemp -d); S=../../src/z80
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > $T/crt0.s
sdasz80 -o $T/crt0.rel $T/crt0.s
for d in $SF_DEFS; do echo "$d = 1"; done > $T/defs.s; L=; [ -n "$SF_DEFS" ] && L='-b _SFLUT=0x7000'
for f in $S/*.s; do b=$(basename $f .s); cp $f $T/$b.s; sdasz80 -g -o $T/$b.rel $T/defs.s $T/$b.s; done
for op in $OPS; do
  case $op in mul) f=__mulsi3;; udiv) f=__udivsi3;; umod) f=__umodsi3;; div) f=__divsi3;; mod) f=__modsi3;; fadd) f=__addsf3;; fsub) f=__subsf3;; fmul) f=__mulsf3;; fdiv) f=__divsf3;; citof) f=__floatsisf;; cutof) f=__floatunsisf;; cftoi) f=__fixsfsi;; cftou) f=__fixunssfsi;; esac
  case $op in f*|c*) G=gen_f32.py; A=${op#?};; *) G=gen.py; A=$op;; esac
  python3 $G $A ${EDGE:+edge} > $T/vec.inc
  sed "s/OP/$f/g" drv.s > $T/drv.s
  printf '\t.module stub\n\t.area _CODE\n\t.globl %s\n%s:\n\tret\n' $f $f > $T/stub.s
  mkdir -p $T/b; python3 basevec.py 4 < $T/vec.inc > $T/b/vec.inc; cp $T/drv.s $T/b/drv.s
  (cd $T/b; sdasz80 -o drv.rel drv.s)
  (cd $T; sdasz80 -o drv.rel drv.s; sdasz80 -o stub.rel stub.s
   sdldz80 -i -b _CODE=0x100 s.ihx crt0.rel b/drv.rel stub.rel >/dev/null
   sdldz80 -i -m -b _CODE=0x100 $L r.ihx crt0.rel drv.rel int32.rel sf32_cmp.rel sf32_arith.rel >/dev/null)
  if [ -n "$L" ]; then c=$(awk '$1=="_CODE" {print "0x" $2 " + 0x" $3}' $T/r.map); t=$(awk '$1=="_SFLUT" {print "0x" $2}' $T/r.map)
    [ $(( $c )) -le $(( $t )) ] || { echo "$op: code and vectors run into the tables at $t"; exit 1; }; fi
  run() { printf 'run\nstate\ndump rom 0x8f00 0x8f01\ndump rom 0x8f10 0x8f1f\nquit\n' | ucsim_z80 -t Z80 -q -w $T/$1.ihx -c - 2>&1; }
  os=$(run s); b=$(echo "$os" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
  fb=$(echo "$os" | sed -n 's/^0x8f00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
  [ "${fb:-ffff}" = 0000 ] || { echo "$op: the stub run must pass its records (basevec.py), got $fb"; exit 1; }
  o=$(run r); c=$(echo "$o" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
  f2=$(echo "$o" | sed -n 's/^0x8f00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
  [ "${f2:-ffff}" != 0000 ] && echo "$o" | grep -E "^0x8f1" | head -3
  nv=$(sed -n 's/^NV = //p' $T/vec.inc)
  echo "$op fails=$((0x${f2:-ffff})) cycles=$(( (c-b)/nv ))"
done
rm -r $T
