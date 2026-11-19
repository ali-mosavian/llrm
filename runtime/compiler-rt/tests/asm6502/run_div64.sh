#!/bin/sh
# usage: tests/asm6502/run_div64.sh [src.s]: config/6502/div64.s (or src.s) against Python integers on sim65: q = floor(ma * 2^56 / mb) and the
# remainder of the 57 steps, every pair of edge mantissas (gen_div64.py edge) and 50000 random pairs, 900 records per run. Prints the
# failing records per set; exits 1 on any. A short loop or a wrong compare fails it; with LLRM_CC set it also checks the hook is wired.
set -e
cd "$(dirname "$0")"; SRC=$(realpath "${1:-../../config/6502/div64.s}"); T=$(mktemp -d)
ca65 -t sim6502 "$SRC" -o $T/m.o
bad=0
for set in edge:289 rnd:50000; do
  k=${set%:*}; n=${set#*:}; f=0; s=0
  while [ $s -lt $n ]; do
    python3 gen_div64.py $k $s 900 > $T/vec.inc
    ca65 -t sim6502 -U -I $T drv_div64.s -o $T/d.o
    ld65 -t sim6502 $T/d.o $T/m.o sim6502.lib -o $T/t.bin
    set +e; sim65 -x 2000000000 $T/t.bin; r=$?; set -e
    f=$((f + r)); s=$((s + 900))
  done
  echo "__sf_div64 $k n=$n fails=$f"; bad=$((bad + f))
done
# With LLRM_CC: f64/div.c calls the kernel under -DSF_DIV64_6502 and not without it (the hook is wired, and off by default).
if [ -n "$LLRM_CC" ]; then
  cd ../..
  for flag in -DSF_DIV64_6502 -UNONE; do
    $LLRM_CC --target=6502-sim65 -Os -S -DSF_INTABI $flag -Iinclude lib/fp/f64/div.c -o $T/sf64.s
    calls=$(grep -c 'jsr.*__sf_div64' $T/sf64.s || true)
    echo "f64/div.c $flag calls __sf_div64: $calls"
    if [ "$flag" = -DSF_DIV64_6502 ]; then [ "$calls" -gt 0 ] || bad=$((bad + 1)); else [ "$calls" -eq 0 ] || bad=$((bad + 1)); fi
  done
fi
rm -r $T
[ $bad -eq 0 ]
