#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [MOS_RT=mos_rt.s] shift_steps_bytes.sh -> reached bytes (reach65.py, -Os, 6502-sim65) of f32 add/mul/div with the library defaults and with config/6502/config.h.
# Fails if any grows: -Os is bytes strict. Absolute caps (the 6502 sizes before sf_round3 existed: add 3103 (2775 since the add preamble), mul 3679, div 2911) catch a default that got
# worse for the 6502 (the add-3 rounding form: add 3109, so config/6502/config.h defines SF_ROUND_BRANCHY). (Stepping every variable shift grew mul +13 B; stepping the addition's alone grows add +18 B, so config/6502/config.h keeps it off at -Os.)
set -e
cd "$(dirname "$0")"; S=../..; T=$(mktemp -d); MOS_RT=${MOS_RT:-$HOME/scratch/handoff/mos_rt.s}
python3 ../woz_compare/gen.py $T 64 >/dev/null
reached() { # op flag
  case $1 in add) f=__addsf3;; mul) f=__mulsf3;; div) f=__divsf3;; esac
  printf '#include <stdint.h>\n#include "ours_%s.h"\nuint32_t %s(uint32_t, uint32_t);\nint main(void) { uint32_t acc = 0; for (unsigned i = 0; i < 64; i++) acc += %s(A[i], B[i]); return acc & 1; }\n' $1 $f $f > $T/m.c
  X=; case "$2" in *6502/config.h*) X=sf32_6502;; esac
  F32=$(cd $S && tests/fp_sources.sh f32 | sed "s#^#$S/#"); N32=$(for s in $F32; do basename $s .c; done)
  for c in $T/m.c $F32 ${X:+$S/config/6502/$X.c}; do $LLRM_CC --target=6502-sim65 -Os -DSF_INTABI $2 -I$S/include -I$T -S $c -o $T/$(basename $c .c).s; done
  { grep -h '^rc[0-9]* = ' $T/m.s $(for x in $N32; do echo $T/$x.s; done) ${X:+$T/$X.s} | sort -u; printf '.export _main\n_main:\n  jsr main\n  rts\n'; for x in m $N32 $X; do grep -v '^rc[0-9]* = ' $T/$x.s || true; done; cat $MOS_RT; } > $T/p.s
  cl65 -t sim6502 -Ln $T/l.txt -o $T/p.bin $T/p.s
  python3 reach65.py $T/p.s $T/l.txt $f | cut -d' ' -f1; }
bad=0
for op in add mul div; do
  off=$(reached $op ""); on=$(reached $op "-include $S/config/6502/config.h"); echo "$op: $off -> $on B"
  case $op in add) cap=2775;; mul) cap=2381;; div) cap=2343;; esac
  [ $on -le $cap ] || bad=1
done
rm -r $T
[ $bad = 0 ] && echo "ok" || { echo "FAIL: a reached size grew"; exit 1; }
