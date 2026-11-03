#!/bin/bash
# m16 -Os linked reach of the integer conversions, one operation per object. Before the split fix_si.c and float_si.c each held two
# operations and a program using one linked both: __fixsfsi 201 B, __fixunssfsi 201, __floatsisf 299, __floatunsisf 299 (compiler-rt: 93/83/284/254); with NaN-to-0 kept (no SF_FIX_NAN_UNDEF in config/x86_m16) the fix pair is 109/92.
cd "$(dirname "$0")/.." || exit 1
out=$(bash tests/m16_split_padding.sh Os) || { echo "$out"; exit 2; }
rc=0
for c in __fixsfsi:95 __fixunssfsi:83 __floatsisf:276 __floatunsisf:216; do
  n=${c%:*}; cap=${c#*:}; got=$(echo "$out" | awk -v n=$n '$1==n{print $2}')
  echo "$n reach ${got:-none} (cap $cap)"; [ -n "$got" ] && [ "$got" -le "$cap" ] || rc=1
done
exit $rc
