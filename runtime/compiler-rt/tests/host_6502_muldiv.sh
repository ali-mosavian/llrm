#!/bin/sh
# usage: tests/host_6502_muldiv.sh -> host_f32 oracle (60M cases) against the 6502 build: the generic f32 files without mul/div + config/6502/sf32_6502.c's mul/div (SF_EXT_MULDIV, config/6502/config.h).
# CTL=1 flips the result sign of the tight mul; the oracle must then fail.
set -e
cd "$(dirname "$0")/.."; T=$(mktemp -d); trap 'rm -r $T' EXIT
[ -n "$CTL" ] && sed 's/return finish(sign, e + 1, s);/return finish(sign ^ SIGN, e + 1, s);/' config/6502/sf32_6502.c > $T/t.c || cp config/6502/sf32_6502.c $T/t.c
cc -O2 -ffp-contract=off -fno-builtin -Iinclude -I config/6502 '-DSF_NAME(n)=sfb_##n' -include stdint.h -include config/6502/config.h tests/host_f32.c $(tests/fp_sources.sh f32 f32_64) $T/t.c -o $T/hf
$T/hf | tail -1
