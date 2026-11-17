#!/bin/bash
# m68k int conversions at -O2 (llrm-cc, SF_INTABI, config/m68k/config.h). fix_si and fix_usi: clocks of the tests/m68k_clocks_fix.c driver (qemu-traced, `cycles --exe`)
# against gcc -O2 on the same source: ours must not exceed gcc. The totals hold shift counts at their bound, the same for both sides. fix_di has no driver: instruction lines.
# KNOWN_OVER cells are over gcc today (row M68K-FIX-USI-CLK, owner w8, exit 2026-11-24; fix_usi closed by M68K-WIDE-SHIFT, the 68020 cells by M68K-BFEXTU): each is held at its measured ceiling and goes when the row closes.
# Env: LLRM_CC_DIR (llrm-cc), CYCLES (the cycles example), GCC_M68K (default ~/scratch/gcc-m68k/root).
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; CY=${CYCLES:?set CYCLES}; R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; export GCC_M68K=$R
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu; B=$R/usr/bin/m68k-linux-gnu
declare -A clk KNOWN_OVER=([m68k-68000/fix_si]=761528); T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen 3000 > $T/vectors.h || exit 2
for t in m68k-68000 m68k-68020; do c=${t#m68k-}
  src() { LLRM_CC_TARGET=$t $D/llrm-cc -O2 -S -DSF_INTABI -include config/m68k/config.h -Iinclude lib/fp/f32/$1.c -o $T/a.s || exit 2; }
  for p in fix_si:0 fix_usi:1; do f=${p%:*}; op=${p#*:}
    src $f
    awk 'BEGIN{print ".text"} /^(__|sf)[A-Za-z0-9_]*:$/ {l=$0; sub(/:$/,"",l); print ".globl " l} {print}' $T/a.s > $T/a.g.s
    $B-as -m$c -o $T/ours.o $T/a.g.s || exit 2
    $B-gcc -msoft-float -m$c -O2 -ffreestanding -DSF_INTABI -include config/m68k/config.h -Iinclude -c lib/fp/f32/$f.c -o $T/gcc.o || exit 2
    for s in ours gcc; do
      $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -DOP=$op -I$T tests/m68k_clocks_fix.c $T/$s.o -o $T/$s$c.exe || exit 2
      clk[$s]=$(CYCLES_CPUS=$c CYCLES_QEMU_CPU=m$c $CY --exe $T/$s$c.exe | awk -F'|' '/\.exe/ {gsub(/[^0-9]/,"",$4); print $4}')
    done
    echo "$t $f ours ${clk[ours]} clk, gcc ${clk[gcc]} clk"
    limit=${KNOWN_OVER[$t/$f]:-${clk[gcc]}}
    [ -n "${clk[ours]}" ] && [ -n "${clk[gcc]}" ] && [ "${clk[ours]}" -le "$limit" ] || { echo "  over its limit $limit"; rc=1; }
  done
  src fix_di; got=$(grep -cE '^\s+[a-z]' $T/a.s); echo "$t fix_di $got lines (cap 90)"; [ "$got" -le 90 ] || rc=1
done; exit $rc
