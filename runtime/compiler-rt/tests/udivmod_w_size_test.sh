#!/bin/sh
# usage: LLRM_CC=llrm-cc tests/udivmod_w_size_test.sh -> the 64-bit divide worker at -Os shifts the divisor by byte and bit steps.
# The variable shifts were libcalls with their pair copies: udivmod_w 1599 B at -Os against 1314 B (int64_division 5601 -> 5283 B). -O2 keeps the libcalls (the steps cost +16% clocks there).
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT; CC=${LLRM_CC:-llrm-cc}; bad=0
for o in Os O2; do
  $CC --target=6502-sim65 -$o -S -w -Iinclude -Ilib/int -Ilib lib/int/udivmod_w.c -o $T/$o.s || { echo "FAIL: compile -$o"; exit 1; }
  eval "n_$o=\$(grep -c 'jsr __\(ashl\|lshr\)si3' $T/$o.s)"
done
[ "$n_Os" = 0 ] || { echo "FAIL: -Os udivmod_w calls the 32-bit shift libcalls $n_Os times"; bad=1; }
[ "$n_O2" -gt 0 ] || { echo "FAIL: -O2 has no shift libcalls: the check does not discriminate"; bad=1; }
[ $bad = 0 ] && echo "udivmod_w_size ok"; exit $bad
