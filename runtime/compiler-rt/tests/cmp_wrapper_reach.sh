#!/bin/bash
# f32 compare entry points at x86 m32 -Os (llrm-cc, INTABI, config/x86_m32): entry + shared core bytes. The wrappers once passed the NaN result as a third argument, which
# the local 2-register convention puts on the stack (push/call/pop/ret): lt/le/eq/ne/cmp 17+87 = 104 B, ge/gt 104 (gcc 87, clang 97..100). Two arguments only: tail jump.
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:-$HOME/scratch/sf-rn-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -Os -DSF_INTABI=1 -include config/x86_m32/config.h -Iinclude -c lib/fp/f32/cmp.c -o $T/a.o || exit 2
sz() { nm -S $T/a.o | awk -v s="$1" '$4==s{print $2}' | while read h; do echo $((16#$h)); done; }
core=$(sz sf_cmp_bits)
for c in __ltsf2:95 __lesf2:95 __eqsf2:95 __nesf2:95 __cmpsf2:95 __gesf2:98 __gtsf2:98; do
  s=${c%:*}; cap=${c#*:}; got=$(( $(sz $s) + core )); echo "$s $got B (cap $cap)"; [ "$got" -le "$cap" ] || rc=1
done; exit $rc
