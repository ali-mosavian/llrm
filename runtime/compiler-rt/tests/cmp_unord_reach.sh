#!/bin/bash
# __unordsf2 lives in its own object (lib/fp/f32/unord.c): a link needing it carries only the NaN test, one needing an ordering compare carries no NaN test.
# Before: unord was a wrapper over the ordering core, 114 B at x86 m32 -Os (compiler-rt best 42); moving the NaN test inline into cmp.c instead grew m16 -Os __ltsf2 253 -> 277; split out it is 226.
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:-$HOME/scratch/sf-rn-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT; rc=0
for p in $(tests/fp_sources.sh f32); do
  LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -Os -DSF_INTABI=1 -include config/x86_m32/config.h -Iinclude -c $p -o $T/$(basename $p .c).o || exit 2
done
o=; for x in $T/*.o; do nm $x | grep -q ' T __unordsf2' && o=$x; done
got=$(size -A $o | awk '/^\.text/{t+=$2} END{print t}'); echo "m32 -Os unord object $(basename $o): $got B (cap 42)"; [ "$got" -le 42 ] || rc=1
if [ -n "$LLRM_OW" ]; then
  r=$(bash tests/m16_split_padding.sh Os | awk '$1=="__ltsf2"{print $2}'); echo "m16 -Os __ltsf2 reach $r (cap 226)"; [ "${r:-999}" -le 226 ] || rc=1
fi
exit $rc
