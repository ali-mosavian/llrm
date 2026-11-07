#!/bin/bash
# Each f32 operation links alone: a one-op program once pulled the whole 1705 B asm (+500..600 B over SDCC). Bounds are bytes of _CODE, add/sub/mul/div at main (750/973: no op grows).
# A conversion that declares a symbol it never uses (.globl done) links the 193 B common member too: cftoi/cftou 337 B, SDCC 562/246.
set -e
cd "$(dirname "$0")/.."
T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT
for m in common round addsub muldiv float fix; do sdasz80 -g -o $T/$m.rel config/z80/sf32_$m.s; done
printf 'common\nround\naddsub\nmuldiv\nfloat\nfix\n' > $T/l.lib
fail=0
for pair in __addsf3:750 __subsf3:750 __mulsf3:973 __divsf3:973 __floatsisf:150 __floatunsisf:150 __fixsfsi:150 __fixunssfsi:150; do
  op=${pair%%:*}; max=${pair##*:}
  printf '\t.area _CODE\n\t.globl %s\nstart:\tcall %s\n' $op $op > $T/t.s; sdasz80 -g -o $T/t.rel $T/t.s
  out=$(sdldz80 -i -m -b _CODE=0x100 $T/t.ihx $T/t.rel -k $T -l l 2>&1) && ! grep -q "ASlink-Warning\|Error" <<<"$out" || { echo "FAIL $op: link: $out"; fail=1; continue; }
  size=$(awk '$1=="_CODE" {sub(/\./,"",$5); print $5; exit}' $T/t.map)
  [ "$size" -le "$max" ] || { echo "FAIL $op: $size B > $max"; fail=1; }
done
[ $fail = 0 ] && echo "ok"; exit $fail
