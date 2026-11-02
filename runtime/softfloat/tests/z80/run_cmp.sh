#!/bin/sh
# usage: tests/z80/run_cmp.sh [ops]   (fle fge funord): src/z80/sf32_cmp.s against the host compare on ucsim_z80

set -e
cd "$(dirname "$0")"; OPS=${*:-fle fge funord}; T=$(mktemp -d); S=../../src/z80
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > $T/crt0.s
sdasz80 -o $T/crt0.rel $T/crt0.s
cp $S/sf32_cmp.s $T/sf32_cmp.s; sdasz80 -g -o $T/sf32_cmp.rel $T/sf32_cmp.s
sz=$(grep -E "^A _CODE size" $T/sf32_cmp.rel | awk '{print strtonum("0x"$4)}' 2>/dev/null || true)
for op in $OPS; do
  case $op in fle) f=__ltsf2;; fge) f=__gtsf2;; funord) f=__unordsf2;; esac
  python3 gen_cmp.py $op > $T/vec.inc
  sed "s/OP/$f/g" drv16.s > $T/drv.s
  printf '\t.module stub\n\t.area _CODE\n\t.globl %s\n%s:\n\tret\n' $f $f > $T/stub.s
  (cd $T; sdasz80 -o drv.rel drv.s; sdasz80 -o stub.rel stub.s
   sdldz80 -i -b _CODE=0x100 s.ihx crt0.rel drv.rel stub.rel >/dev/null
   sdldz80 -i -b _CODE=0x100 r.ihx crt0.rel drv.rel sf32_cmp.rel >/dev/null)
  run() { printf 'run\nstate\ndump rom 0x8f00 0x8f01\nquit\n' | ucsim_z80 -t Z80 -q -w $T/$1.ihx -c - 2>&1; }
  b=$(run s | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
  o=$(run r); c=$(echo "$o" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
  f2=$(echo "$o" | sed -n 's/^0x8f00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
  nv=$(sed -n 's/^NV = //p' $T/vec.inc)
  echo "$op fails=$((0x${f2:-ffff})) cycles=$(( (c-b)/nv ))"
done
rm -r $T
