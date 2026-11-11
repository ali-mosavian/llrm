#!/bin/sh
# The f64 quotient in hardware digits (SF_DIV64_HW32: six 32/32; SF_DIV64_UDIV: two 64/32 through SF_UDIV64BY32) must match the host FPU bit for bit, like the default loop (a digit estimate off by more than the correction loops cover would show here).
cd "$(dirname "$0")/.."
T=$(mktemp -d); trap 'rm -r $T' EXIT
for knob in SF_DIV64_HW32 SF_DIV64_UDIV; do
  cc -O2 -fno-builtin -w -D$knob=1 -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f64.c lib/fp/f64/*.c -o $T/h || exit 2
  $T/h "${1:-300000}" > $T/out
  grep -E '^div' $T/out | tail -1
  grep -q 'fails=0' $T/out || { echo "FAIL: $knob differs from the host FPU"; exit 1; }
done
echo ok
