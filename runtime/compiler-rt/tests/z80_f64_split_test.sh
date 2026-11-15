#!/bin/bash
# usage: LLRM_Z80_LLC=<llrm-z80-llc> tests/z80_f64_split_test.sh
# A program linking one f64 compare or int-to-float entry links that entry alone: sdld takes an object whole, and float.c / cmp.c held all of
# theirs (__floatsidf linked 1758 B, __eqdf2 1136 B); add, mul and div each carried a copy of the 208 B decode, so the three linked 9142 B; the shared decode links 8916 B.
# The compiled 64x64 product spilled every word: __muldf3 alone linked 3934 B, 3335 B with config/z80/mul64.s; the three 8916 B, 8317 B. Bounds are bytes of _CODE.
set -e
cd "$(dirname "$0")/.."
T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT
bash "$PWD/tests/z80_lib.sh" $T/lib > $T/build.log 2>&1 || { cat $T/build.log; exit 1; }
ls $T/lib/*.rel | xargs -n1 basename | sed 's/\.rel$//' > $T/lib/l.lib
# the shifts the runtime provides, one byte each: the bounds are the library's own
printf '\t.area _CODE\n\t.globl __ashlsi3, __lshrsi3\n__ashlsi3:\n__lshrsi3:\tret\n' > $T/stub.s; sdasz80 -g -o $T/stub.rel $T/stub.s
fail=0
for pair in __adddf3,__muldf3,__divdf3:8400 __muldf3:3400 __eqdf2:820 __nedf2:820 __ltdf2:820 __ledf2:820 __gedf2:830 __gtdf2:910 __floatsidf:800 __floatunsidf:400; do
  op=${pair%%:*}; max=${pair##*:}
  { printf '\t.area _CODE\n'; for o in ${op//,/ }; do printf '\t.globl %s\n' $o; done; printf 'start:'; for o in ${op//,/ }; do printf '\tcall %s\n' $o; done; } > $T/t.s; sdasz80 -g -o $T/t.rel $T/t.s
  out=$(sdldz80 -i -m -b _CODE=0x100 $T/t.ihx $T/t.rel $T/stub.rel -k $T/lib -l l 2>&1) && ! grep -q "ASlink-Warning\|Error" <<<"$out" || { echo "FAIL $op: link: $out"; fail=1; continue; }
  size=$(awk '$1=="_CODE" {sub(/\./,"",$5); print $5; exit}' $T/t.map)
  echo "$op: $size B (max $max)"
  [ "$size" -le "$max" ] || { echo "FAIL $op: $size B > $max"; fail=1; }
done
[ $fail = 0 ] && echo "ok"; exit $fail
