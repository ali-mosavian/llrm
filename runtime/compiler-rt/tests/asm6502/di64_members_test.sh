#!/bin/sh
# A divide-only program links its own member: udiv/sdiv no larger than #7830's single di64.o (945 B CODE), umod/mod (own member, no quotient roll) far smaller.
set -e
cd "$(dirname "$0")"; C=../../config/6502; T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT; fail=0
ca65 -t sim6502 $C/di64.s -o $T/di64.o; ca65 -t sim6502 $C/umod64.s -o $T/umod64.o
for e in di64:__udivdi3:945 di64:__divdi3:945 umod64:__umoddi3:700 umod64:__moddi3:700; do
  m=${e%%:*}; r=${e#*:}; f=${r%%:*}; max=${r##*:}
  printf '.import %s\n.export _main\n_main: jsr %s\nrts\n' $f $f > $T/t.s; ca65 -t sim6502 $T/t.s -o $T/t.o
  ld65 -t sim6502 $T/t.o $T/$m.o sim6502.lib -m $T/t.map -o $T/t.bin
  n=$(awk '/^CODE /{print $4; exit}' $T/t.map); n=$(printf '%d' 0x$n)
  [ "$n" -le "$max" ] || { echo "FAIL $f: $n B > $max"; fail=1; }
done
[ $fail = 0 ] && echo ok; exit $fail
