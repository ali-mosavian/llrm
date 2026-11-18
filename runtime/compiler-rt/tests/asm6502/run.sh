#!/bin/sh
# usage: [EDGE=1] [SF_DEFS='-DSF_MUL_LUT=1 ...'] tests/asm6502/run.sh [n] [ops]  (n <= 4500): bit-exact run of config/6502 on sim65 against the host FPU; prints fails
# ops: add sub mul div itof utof ftoi ftou (default all). Run under taskset when the machine is loaded.
set -e
cd "$(dirname "$0")"; N=${1:-3000}; OPS=${2:-add sub mul div itof utof ftoi ftou}; T=$(mktemp -d); I=../../config/6502
S=; X=$T/sf32_abi.o
for f in sf32 sf32_muldiv sf32_conv sf32_abi; do ca65 -t sim6502 -I $I $S $SF_DEFS $I/$f.s -o $T/$f.o; done
for op in $OPS; do
  python3 gen.py $op $N ${EDGE:+edge}${DIRECTED:+directed} > $T/vec.inc
  case $op in add) S=__addsf3;; sub) S=__subsf3;; mul) S=__mulsf3;; div) S=__divsf3;; itof) S=__floatsisf;; utof) S=__floatunsisf;; ftoi) S=__fixsfsi;; ftou) S=__fixunssfsi;; esac
  case $op in itof|utof|ftoi|ftou) D=-DEXACT=1;; *) D=${DIRECTED:+-DEXACT=1};; esac
  printf '.import %s\nOP = %s\n' $S $S > $T/op.inc
  ca65 -t sim6502 -U -I $T $D drv.s -o $T/drv.o
  ld65 -C sim6502_lut.cfg $T/drv.o $T/sf32.o $T/sf32_muldiv.o $T/sf32_conv.o $X sim6502.lib -o $T/t.bin
  set +e; sim65 -c $T/t.bin; echo "$op n=$N fails=$?"; set -e
done
rm -r $T
