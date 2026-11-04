#!/bin/sh
# usage: bench_llvmmos.sh [OPT] -> llvm-mos float routines (compiler-rt softfloat) on the Woz comparison operands, same 64 records as
# bench.sh: extra bytes (text+rodata of the op build minus a plain copy), mean cycles per call (minus the copy), mismatches vs the IEEE result.
set -e
cd "$(dirname "$0")"; OPT=${1:--Os}; T=$(mktemp -d); B=${LLVM_MOS:-$HOME/scratch/fetch-llvm-mos-sdk/x/llvm-mos}/bin
python3 ../woz_compare/gen.py $T 64 >/dev/null
for op in add sub mul div; do
  case $op in add) e='x.f + y.f';; sub) e='x.f - y.f';; mul) e='x.f * y.f';; div) e='x.f / y.f';; esac
  cat > $T/m.c <<E2
#include <stdint.h>
#include <stdio.h>
#include "ours_$op.h"
typedef union { float f; uint32_t u; } U;
int main(void) {
  unsigned fails = 0;
  for (unsigned i = 0; i < 64; i++) {
    U x, y, r; x.u = A[i]; y.u = B[i];
#ifdef BASE
    r.u = x.u;
#else
    r.f = $e;
#endif
    if (r.u != W[i] && !(((r.u << 1) > 0xFF000000u) && ((W[i] << 1) > 0xFF000000u))) fails++;
  }
  return fails > 255 ? 255 : fails;
}
E2
  for k in base full; do
    d=""; [ $k = base ] && d=-DBASE
    $B/mos-sim-clang $OPT $d -I$T -o $T/$k $T/m.c 2>$T/err || { cat $T/err; exit 1; }
    set +e; $B/mos-sim --cycles $T/$k 2>$T/$k.out >/dev/null; eval "f_$k=\$?"; set -e
    eval "c_$k=\$(sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' $T/$k.out | head -1)"
    eval "s_$k=\$($B/llvm-size $T/$k.elf | awk 'NR==2{print \$1}')"
  done
  echo "$op llvm-mos $OPT: +bytes=$(( s_full - s_base )) fails=$f_full cycles=$(( (c_full - c_base) / 64 ))"
done
rm -r $T
