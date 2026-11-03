#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [MOS_RT=mos_rt.s] shift_steps_bytes.sh -> reached bytes (reach65.py, -Os, 6502-sim65) of f32 add/mul/div with the library defaults and with include/sf_6502.h.
# Fails if any grows: -Os is bytes strict. (Stepping every variable shift grew mul +13 B; stepping the addition's alone grows add +18 B, so sf_6502.h keeps it off at -Os.)
set -e
cd "$(dirname "$0")"; S=../..; T=$(mktemp -d); MOS_RT=${MOS_RT:-$HOME/scratch/handoff/mos_rt.s}
python3 ../woz_compare/gen.py $T 64 >/dev/null
reached() { # op flag
  case $1 in add) f=__addsf3;; mul) f=__mulsf3;; div) f=__divsf3;; esac
  printf '#include <stdint.h>\n#include "ours_%s.h"\nuint32_t %s(uint32_t, uint32_t);\nint main(void) { uint32_t acc = 0; for (unsigned i = 0; i < 64; i++) acc += %s(A[i], B[i]); return acc & 1; }\n' $1 $f $f > $T/m.c
  for c in $T/m.c $S/src/sf32.c $S/src/sf32_conv.c; do $LLRM_CC --target=6502-sim65 -Os -DSF_INTABI $2 -I$S/include -I$T -S $c -o $T/$(basename $c .c).s; done
  { grep -h '^rc[0-9]* = ' $T/m.s $T/sf32.s $T/sf32_conv.s | sort -u; printf '.export _main\n_main:\n  jsr main\n  rts\n'; for x in m sf32 sf32_conv; do grep -v '^rc[0-9]* = ' $T/$x.s; done; cat $MOS_RT; } > $T/p.s
  cl65 -t sim6502 -Ln $T/l.txt -o $T/p.bin $T/p.s
  python3 reach65.py $T/p.s $T/l.txt $f | cut -d' ' -f1; }
bad=0
for op in add mul div; do
  off=$(reached $op ""); on=$(reached $op "-include $S/include/sf_6502.h"); echo "$op: $off -> $on B"
  [ $on -le $off ] || bad=1
done
rm -r $T
[ $bad = 0 ] && echo "ok" || { echo "FAIL: a reached size grew"; exit 1; }
