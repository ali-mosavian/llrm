#!/bin/bash
# x86 m32 (llrm-cc x86_32-linux, config/x86_m32/config.h) object bytes of the int conversions, -Os. Without SF_FIX_NAN_UNDEF in the m32 config the
# NaN-to-0 checks give fix_si 105 and fix_usi 81 (compiler-rt best: 92 and 85); the one-magnitude floatsisf gave float_si 116 -> 115 at -Os, 163 -> 117 at -O2.
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:-$HOME/scratch/sf-rn-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
for c in fix_si:94 fix_usi:74 float_si:115 float_usi:87; do
  p=${c%:*}; cap=${c#*:}
  LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -Os -DSF_INTABI=1 -include config/x86_m32/config.h -Iinclude -c lib/fp/f32/$p.c -o $T/a.o || exit 2
  got=$(size -A $T/a.o | awk '/^\.text/{t+=$2} END{print t}'); echo "$p $got B (cap $cap)"; [ "$got" -le "$cap" ] || rc=1
done
exit $rc
