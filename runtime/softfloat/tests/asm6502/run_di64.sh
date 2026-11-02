#!/bin/sh
# usage: tests/asm6502/run_di64.sh [n] [ops]  (n <= 2500): src/6502/di64.s against Python integers, 6502 call layout, on sim65
set -e
cd "$(dirname "$0")"; N=${1:-1000}; OPS=${2:-mul}; T=$(mktemp -d); I=../../src/6502
ca65 -t sim6502 $I/di64.s -o $T/di64.o
for op in $OPS; do
  case $op in mul) S=__muldi3;; udiv) S=__udivdi3;; umod) S=__umoddi3;; div) S=__divdi3;; mod) S=__moddi3;; esac
  python3 gen_di64.py $op $N > $T/vec.inc; printf '.import %s\nOP = %s\n' $S $S > $T/op.inc
  ca65 -t sim6502 -U -I $T drv_di64.s -o $T/drv.o
  ld65 -t sim6502 $T/drv.o $T/di64.o sim6502.lib -o $T/t.bin
  set +e; sim65 -c -x 3000000000 $T/t.bin; echo "$S n=$N fails=$?"; set -e
done
rm -r $T
