#!/bin/sh
# usage: LLRM_CC_DIR=dir tests/m68k_div_hw.sh -> with config/m68k/config.h sf32.c divides with divu.l on the 68020 (4 per division) and not on the 68000,
# whose 32-bit divide is a library call. Fails if the 68020 keeps the 28-step loop (541 instructions per division against 127).
set -e
cd "$(dirname "$0")/.."; D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; T=$(mktemp -d); trap 'rm -r $T' EXIT
n() { LLRM_CC_TARGET=m68k-$1 $D/llrm-cc -Os -S -Iinclude -DSF_INTABI=1 -include config/m68k/config.h lib/fp/f32/div.c -o $T/s.s; grep -ci 'divu\.l' $T/s.s || true; }
a=$(n 68020); b=$(n 68000)
[ "$a" = 4 ] && [ "$b" = 0 ] || { echo "FAIL divu.l: 68020 $a (want 4), 68000 $b (want 0)"; exit 1; }
echo "ok: divu.l 68020 $a, 68000 $b"
