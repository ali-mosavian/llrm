#!/bin/sh
# usage: [OLDSHIM=1] tests/asm6502/run.sh [n] [ops]  (n <= 4500): bit-exact run of src/6502 on sim65 against the host FPU; prints fails
# ops: add sub mul div itof utof ftoi ftou (default all). Run under taskset when the machine is loaded.
set -e
cd "$(dirname "$0")"; N=${1:-3000}; OPS=${2:-add sub mul div itof utof ftoi ftou}; T=$(mktemp -d); I=../../src/6502
if [ -n "$OLDSHIM" ]; then S=; X=$T/sf32_abi.o; else S=-DSTACKB=1; X=; fi
for f in sf32 sf32_muldiv sf32_conv sf32_abi; do ca65 -t sim6502 -I $I $S $I/$f.s -o $T/$f.o; done
for op in $OPS; do
  python3 gen.py $op $N > $T/vec.inc
  case $op in add) S=__addsf3;; sub) S=__subsf3;; mul) S=__mulsf3;; div) S=__divsf3;; itof) S=__floatsisf;; utof) S=__floatunsisf;; ftoi) S=__fixsfsi;; ftou) S=__fixunssfsi;; esac
  case $op in itof|utof|ftoi|ftou) D=-DEXACT=1;; *) D=;; esac
  printf '.import %s\nOP = %s\n' $S $S > $T/op.inc
  ca65 -t sim6502 -U -I $T $D drv.s -o $T/drv.o
  ld65 -t sim6502 $T/drv.o $T/sf32.o $T/sf32_muldiv.o $T/sf32_conv.o $X sim6502.lib -o $T/t.bin
  set +e; sim65 -c $T/t.bin; echo "$op n=$N fails=$?"; set -e
done
rm -r $T
