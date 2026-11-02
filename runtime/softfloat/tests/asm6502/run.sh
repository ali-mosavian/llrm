#!/bin/sh
# usage: tests/asm6502/run.sh [n] [ops]  (n <= 4500): bit-exact run of src/6502 add/sub/mul/div on sim65; prints fails and cycles
set -e
cd "$(dirname "$0")"; N=${1:-3000}; OPS=${2:-add sub mul div}; T=$(mktemp -d); I=../../src/6502
ca65 -t sim6502 -I $I $I/sf32.s -o $T/sf.o; ca65 -t sim6502 -I $I $I/sf32_muldiv.s -o $T/md.o
for op in $OPS; do
  python3 gen.py $op $N > $T/vec.inc
  case $op in sub) D=-DSUBOP=1;; mul) D=-DMULOP=1;; div) D=-DDIVOP=1;; *) D=;; esac
  ca65 -t sim6502 -U -I $T $D drv.s -o $T/drv.o
  ld65 -t sim6502 $T/drv.o $T/sf.o $T/md.o sim6502.lib -o $T/t.bin
  set +e; sim65 -c $T/t.bin; echo "$op n=$N fails=$?"; set -e
done
rm -r $T
