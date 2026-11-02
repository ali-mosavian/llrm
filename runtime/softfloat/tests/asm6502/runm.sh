#!/bin/sh
# usage: tests/asm6502/runm.sh [n]  (n <= 4500): src/6502/sf32_explog.s expf/logf vs the C reference bit for bit on sim65
set -e
cd "$(dirname "$0")"; N=${1:-1500}; T=$(mktemp -d); I=../../src/6502; S=../..
cc -O2 -ffp-contract=off -fno-builtin -I$S/include '-DSF_NAME(n)=sfb_##n' '-DSFM_NAME(n)=sfb_##n' genm.c $S/src/mathf.c $S/src/sf32_conv.c $S/src/sf32.c $S/src/sf32_conv64.c -lm -o $T/genm
for f in sf32 sf32_muldiv sf32_conv sf32_explog; do ca65 -t sim6502 -I $I $I/$f.s -o $T/$f.o; done
for op in exp log; do
  $T/genm $op $N > $T/vec.inc
  printf '.import %sf\nOP = %sf\n' $op $op > $T/op.inc
  ca65 -t sim6502 -U -I $T -DEXACT=1 drv.s -o $T/drv.o
  ld65 -t sim6502 $T/drv.o $T/sf32.o $T/sf32_muldiv.o $T/sf32_conv.o $T/sf32_explog.o sim6502.lib -o $T/t.bin
  set +e; sim65 -c -x 3000000000 $T/t.bin; echo "${op}f n=$N fails=$?"; set -e
done
rm -r $T
