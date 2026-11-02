#!/bin/sh
# usage: tests/asm6502/run8.sh [ops]: exhaustive check (all 65536 pairs) of src/6502/sf8.s against exact rational arithmetic
set -e
cd "$(dirname "$0")"; OPS=${1:-add sub mul div}; T=$(mktemp -d); I=../../src/6502
ca65 -t sim6502 -I $I $I/sf8.s -o $T/sf8.o
for op in $OPS; do
  printf '.import __fp8_%s\nOP = __fp8_%s\n' $op $op > $T/op.inc; tot=0
  for c in $(seq 0 15); do
    python3 gen8.py $op $c > $T/exp8.inc
    ca65 -t sim6502 -U -I $T drv8.s -o $T/drv.o
    ld65 -t sim6502 $T/drv.o $T/sf8.o sim6502.lib -o $T/t.bin
    set +e; sim65 -c $T/t.bin >/dev/null; f=$?; set -e
    [ $f -ne 0 ] && echo "$op chunk $c: $f mismatches"
    tot=$((tot + f))
  done
  echo "$op pairs=65536 mismatches=$tot"
done
rm -r $T
