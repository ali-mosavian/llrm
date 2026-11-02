#!/bin/sh
# usage: tests/asm6502/run.sh [n]  (n <= 4500): bit-exact run of src/6502/sf32.s add and sub on sim65; prints failures and cycles
set -e
cd "$(dirname "$0")"; N=${1:-3000}; T=$(mktemp -d)
ca65 -t sim6502 ../../src/6502/sf32.s -o $T/sf.o
for op in add sub; do
  python3 gen.py $op $N > $T/vec.inc
  ca65 -t sim6502 -U -I $T $( [ $op = sub ] && echo -DSUBOP=1 ) drv.s -o $T/drv.o
  ld65 -t sim6502 $T/drv.o $T/sf.o sim6502.lib -o $T/t.bin
  set +e; sim65 -c $T/t.bin; echo "$op n=$N fails=$?"; set -e
done
rm -r $T
