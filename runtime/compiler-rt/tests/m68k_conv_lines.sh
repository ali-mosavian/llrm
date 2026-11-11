#!/bin/bash
# m68k (llrm-cc, SF_INTABI, config/m68k/config.h) instruction lines of the int conversions at -O2. Without SF_FIX_NAN_UNDEF: fix_si 39, fix_usi 34, fix_di 197.
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:-$HOME/scratch/sf-rn-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
for t in m68k-68000 m68k-68020; do for c in fix_si:34 fix_usi:30 fix_di:181; do
  p=${c%:*}; cap=${c#*:}
  LLRM_CC_TARGET=$t $D/llrm-cc -O2 -S -DSF_INTABI -include config/m68k/config.h -Iinclude lib/fp/f32/$p.c -o $T/a.s || exit 2
  got=$(grep -cE '^\s+[a-z]' $T/a.s); echo "$t $p $got lines (cap $cap)"; [ "$got" -le "$cap" ] || rc=1
done; done; exit $rc
