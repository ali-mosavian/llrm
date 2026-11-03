#!/bin/bash
# f64 -> int conversions at x86 m32 -Os (llrm-cc, SF_INTABI, config/x86_m32): the object that defines each entry point, one entry per object.
# Before: one fix.c with a bits-parametrised 64-bit core: __fixdfsi 17+136+127 = 280 B linked, __fixdfdi 280, __fixunsdfsi 184, __fixunsdfdi 200 (compiler-rt best 108/59/95/109).
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:-$HOME/scratch/sf-rn-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
for f in lib/fp/f64/*.c; do LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -Os -DSF_INTABI=1 -include config/x86_m32/config.h -Iinclude -c $f -o $T/$(basename $f .c).o 2>/dev/null; done
for c in __fixdfsi:80 __fixunsdfsi:67 __fixdfdi:132 __fixunsdfdi:99; do
  s=${c%:*}; cap=${c#*:}; o=; for x in $T/*.o; do nm $x 2>/dev/null | grep -q " T $s\$" && o=$x; done
  got=$(size -A $o 2>/dev/null | awk '/^\.text/{t+=$2} END{print t+0}'); echo "$s object $(basename "$o"): $got B (cap $cap)"; [ "$got" -gt 0 ] && [ "$got" -le "$cap" ] || rc=1
done; exit $rc
