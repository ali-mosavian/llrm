#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [MOS_RT=mos_rt.s] bench_llrmcc.sh [OPT] -> our C path (llrm-cc 6502-sim65, sf32.c) on the Woz operands, as bench_llvmmos.sh:
# extra bytes (text of the op build minus a plain copy), mean sim65 cycles per call (minus the copy), mismatches vs the IEEE result.
set -e
cd "$(dirname "$0")"; OPT=${1:-Os}; T=${KEEP:-$(mktemp -d)}; S=../..; MOS_RT=${MOS_RT:-$HOME/scratch/handoff/mos_rt.s}
python3 ../woz_compare/gen.py $T 64 >/dev/null
for op in add sub mul div; do
  case $op in add) f=__addsf3;; sub) f=__subsf3;; mul) f=__mulsf3;; div) f=__divsf3;; esac
  cat > $T/m.c <<E2
#include <stdint.h>
#include "ours_$op.h"
uint32_t $f(uint32_t, uint32_t);
int main(void) {
  unsigned fails = 0;
  for (unsigned i = 0; i < 64; i++) {
    uint32_t r;
#ifdef BASE
    r = A[i];
#else
    r = $f(A[i], B[i]);
#endif
    if (r != W[i] && !(((r << 1) > 0xFF000000u) && ((W[i] << 1) > 0xFF000000u))) fails++;
  }
  return fails > 255 ? 255 : fails;
}
E2
  for k in base full; do
    d=""; [ $k = base ] && d=-DBASE
    srcs="$T/m.c"; [ $k = full ] && srcs="$T/m.c $S/src/sf32.c $S/src/sf32_conv.c"
    # the 6502 config takes mul/div from sf32_6502.c
    case "$op:${SF6502--include $S/include/sf_6502.h}" in mul:?*|div:?*) [ $k = full ] && grep -q "define SF_EXT_MULDIV" ${SF6502_HDR:-$S/include/sf_6502.h} && srcs="$srcs $S/src/sf32_6502.c";; esac
    : > $T/all.s
    for c in $srcs; do
      $LLRM_CC --target=6502-sim65 -$OPT $d -DSF_INTABI ${SF6502--include $S/include/sf_6502.h} -I$S/include -I$T -S $c -o $T/$(basename $c .c).s
    done
    { grep -h '^rc[0-9]* = ' $T/*.s | sort -u; printf '.export _main\n_main:\n  jsr main\n  rts\n'; for c in $srcs; do grep -v '^rc[0-9]* = ' $T/$(basename $c .c).s; done; cat $MOS_RT; } > $T/p.s
    cl65 -t sim6502 -m $T/$k.map -o $T/$k.bin $T/p.s 2>$T/err || { cat $T/err; exit 1; }
    set +e; sim65 -c -x 500000000 $T/$k.bin >$T/$k.out 2>/dev/null; eval "f_$k=\$?"; set -e
    eval "c_$k=\$(sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' $T/$k.out | head -1)"
    eval "s_$k=\$(awk '/^(CODE|RODATA) /{print \$4}' $T/$k.map | while read h; do printf '%d\\n' 0x\$h; done | paste -sd+ | bc)"
  done
  echo "$op llrm-cc -$OPT: +bytes=$(( s_full - s_base )) fails=$f_full cycles=$(( (c_full - c_base) / 64 ))"
done
[ -n "$KEEP" ] || rm -r $T
