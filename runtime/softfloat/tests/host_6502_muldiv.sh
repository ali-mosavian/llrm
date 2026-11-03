#!/bin/sh
# usage: tests/host_6502_muldiv.sh -> host_f32 oracle (60M cases) against the 6502 build: generic sf32.c without mul/div + sf32_6502.c's mul/div (SF_EXT_MULDIV, include/sf_6502.h).
# CTL=1 flips the result sign of the tight mul; the oracle must then fail.
set -e
cd "$(dirname "$0")/.."; T=$(mktemp -d); trap 'rm -r $T' EXIT
[ -n "$CTL" ] && sed 's/return finish(sign, e + 1, s);/return finish(sign ^ SIGN, e + 1, s);/' src/sf32_6502.c > $T/t.c || cp src/sf32_6502.c $T/t.c
cc -O2 -ffp-contract=off -fno-builtin -Iinclude -I src '-DSF_NAME(n)=sfb_##n' -include stdint.h -include include/sf_6502.h tests/host_f32.c src/sf32.c src/sf32_conv.c src/sf32_conv64.c $T/t.c -o $T/hf
$T/hf | tail -1
