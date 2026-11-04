#!/bin/sh
# The f64 quotient in 32/32 hardware digits (SF_DIV64_HW32) must match the host FPU bit for bit, like the default loop (a digit estimate off by more than the correction loops cover would show here).
cd "$(dirname "$0")/.."
T=$(mktemp -d); trap 'rm -r $T' EXIT
cc -O2 -fno-builtin -w -DSF_DIV64_HW32=1 -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f64.c lib/fp/f64/*.c -o $T/h || exit 2
$T/h "${1:-300000}" | grep -E '^div|fails' | tail -2
$T/h "${1:-300000}" | grep -q 'fails=0' || { echo "FAIL: SF_DIV64_HW32 differs from the host FPU"; exit 1; }
