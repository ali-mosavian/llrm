#!/bin/bash
# 6502 (llrm-cc, SF_INTABI, config/6502/config.h) instruction lines of the int conversions. Without SF_FIX_NAN_UNDEF at -Os: fix_si 230, fix_usi 176.
cd "$(dirname "$0")/../.." || exit 1
D=${LLRM_CC_DIR:-$HOME/scratch/sf-rn-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
for c in fix_si:179 fix_usi:122; do
  p=${c%:*}; cap=${c#*:}
  LLRM_CC_TARGET=6502-sim65 $D/llrm-cc -Os -S -DSF_INTABI -include config/6502/config.h -Iinclude lib/fp/f32/$p.c -o $T/a.s || exit 2
  got=$(grep -cE '^\s+[a-z]' $T/a.s); echo "6502 -Os $p $got lines (cap $cap)"; [ "$got" -le "$cap" ] || rc=1
done; exit $rc
