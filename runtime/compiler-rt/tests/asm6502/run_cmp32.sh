#!/bin/sh
# usage: tests/asm6502/run_cmp32.sh [src.s]: config/6502/cmp32.s (or src.s) against Python's float compare on sim65, every pair of special
# values (gen_cmp32.py edge) and 50000 random bit patterns (not with EDGE_ONLY=1), 900 records per run. Prints the failing records per
# set; exits 1 on any.
set -e
cd "$(dirname "$0")"; SRC=$(realpath "${1:-../../config/6502/cmp32.s}"); T=$(mktemp -d)
ca65 -t sim6502 "$SRC" -o $T/m.o
bad=0
sets=edge; [ -z "$EDGE_ONLY" ] && sets="edge rnd"
for k in $sets; do
  n=$(python3 gen_cmp32.py $k 0 100000 | sed -n 's/^nrec = //p')
  f=0; s=0
  while [ $s -lt $n ]; do
    python3 gen_cmp32.py $k $s 900 > $T/vec.inc
    ca65 -t sim6502 -U -I $T drv_cmp32.s -o $T/d.o
    ld65 -t sim6502 $T/d.o $T/m.o sim6502.lib -o $T/t.bin
    set +e; sim65 -x 2000000000 $T/t.bin; r=$?; set -e
    f=$((f + r)); s=$((s + 900))
  done
  echo "cmp32 $k n=$n fails=$f"; bad=$((bad + f))
done
rm -r $T
[ $bad -eq 0 ]
