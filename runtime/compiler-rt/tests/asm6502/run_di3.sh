#!/bin/sh
# usage: tests/asm6502/run_di3.sh [n]  (n <= 3000): config/6502/di3.s against Python integers, with the 6502 call layout, on sim65
set -e
cd "$(dirname "$0")"; N=${1:-1500}; T=$(mktemp -d); I=../../config/6502
ca65 -t sim6502 $I/di3.s -o $T/di3.o
for pair in shl:__ashldi3 shr:__lshrdi3 sar:__ashrdi3; do
  op=${pair%%:*}; sym=${pair##*:}
  python3 gen_di3.py $op $N > $T/vec.inc; printf 'OP = %s\n' $sym > $T/op.inc
  ca65 -t sim6502 -U -I $T drv_di3.s -o $T/drv.o
  ld65 -t sim6502 $T/drv.o $T/di3.o sim6502.lib -o $T/t.bin
  set +e; sim65 -c $T/t.bin; echo "$sym n=$N fails=$?"; set -e
done
rm -r $T
