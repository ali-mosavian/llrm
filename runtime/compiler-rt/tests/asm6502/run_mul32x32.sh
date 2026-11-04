#!/bin/sh
# usage: tests/asm6502/run_mul32x32.sh [src.s]: config/6502/mul32x32.s (or src.s) against Python integers on sim65, every byte-pattern pair
# (gen_mul32x32.py pat) and 100000 random pairs, 3000 records per run. Prints the failing records per set; exits 1 on any.
set -e
cd "$(dirname "$0")"; SRC=$(realpath "${1:-../../config/6502/mul32x32.s}"); T=$(mktemp -d)
ca65 -t sim6502 "$SRC" -o $T/m.o
bad=0
for set in pat:390625 rnd:100000; do
  k=${set%:*}; n=${set#*:}; f=0; s=0
  while [ $s -lt $n ]; do
    python3 gen_mul32x32.py $k $s 3000 > $T/vec.inc
    ca65 -t sim6502 -U -I $T drv_mul32x32.s -o $T/d.o
    ld65 -t sim6502 $T/d.o $T/m.o sim6502.lib -o $T/t.bin
    set +e; sim65 -x 2000000000 $T/t.bin; r=$?; set -e
    f=$((f + r)); s=$((s + 3000))
  done
  echo "__mul32x32 $k n=$n fails=$f"; bad=$((bad + f))
done
rm -r $T
[ $bad -eq 0 ]
