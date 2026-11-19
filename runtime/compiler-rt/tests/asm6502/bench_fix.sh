#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [MOS_RT=mos_rt.s] bench_fix.sh [OPT] -> sim65 cycles per call of __fixsfsi / __fixunssfsi, with SF_FIX_NAN_UNDEF (config/6502/config.h) and without,
# over the finite in-range vectors of gen_vectors.c (a NaN or a magnitude >= 2^31 is undefined in C and skipped), minus a loop that copies the operand.
set -e
cd "$(dirname "$0")/../.."; OPT=${1:-Os}; T=$(mktemp -d); trap 'rm -r $T' EXIT; MOS_RT=${MOS_RT:-$HOME/scratch/handoff/mos_rt.s}
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen 200 > $T/vectors.h
sed '/SF_FIX_NAN_UNDEF/d' config/6502/config.h > $T/off.h
for op in fix_si:__fixsfsi fix_usi:__fixunssfsi; do
  file=${op%:*}; f=${op#*:}
  for k in base on off; do
    case $k in base) d=-DBASE; h=config/6502/config.h;; on) d=; h=config/6502/config.h;; off) d=; h=$T/off.h;; esac
    cat > $T/m.c <<E2
#include <stdint.h>
#include "vectors.h"
uint32_t $f(uint32_t);
#define BAD(a) ((((a) >> 23) & 0xFF) >= 158)
int main(void) {
  uint32_t acc = 0;
  for (unsigned i = 0; i < NV; i++) {
    uint32_t a = V[i][0];
    if (BAD(a)) continue;
#ifdef BASE
    acc += a;
#else
    acc += $f(a);
#endif
  }
  return acc & 1;
}
E2
    srcs="$T/m.c lib/fp/f32/$file.c"
    for c in $srcs; do $LLRM_CC --target=6502-sim65 -$OPT $d -DSF_INTABI -include $h -Iinclude -I$T -S $c -o $T/$(basename $c .c).s; done
    { grep -h '^rc[0-9]* = ' $T/*.s | sort -u; printf '.export _main\n_main:\n  jsr main\n  rts\n'; for c in $srcs; do grep -v '^rc[0-9]* = ' $T/$(basename $c .c).s || true; done; cat $MOS_RT; } > $T/p.s
    cl65 -t sim6502 -o $T/$k.bin $T/p.s
    sim65 -c -x 500000000 $T/$k.bin > $T/$k.out 2>/dev/null || true
    eval "c_$k=\$(sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' $T/$k.out | head -1)"
  done
  echo "$f -$OPT: base $c_base  NaN-undef on $c_on  off $c_off  (net on $((c_on - c_base)), off $((c_off - c_base)))"
done
