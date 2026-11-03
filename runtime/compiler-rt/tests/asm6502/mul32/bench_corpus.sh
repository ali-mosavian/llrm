#!/bin/sh
# usage: bench_corpus.sh -> 32x32->32 multiply, a*b in C, same 64 vectors as bench.sh: cc65 (sim65) and llvm-mos (mos-sim) at several levels.
# bytes = image text/code minus the build whose multiply is a plain copy (library helper included); cycles per call = (run - copy run) / 64; fails vs a*b.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); B=${LLVM_MOS:-$HOME/scratch/fetch-llvm-mos-sdk/x/llvm-mos}/bin
python3 - > $T/vec.h <<'P'
import random
random.seed(7)
v=[(0,0),(1,1),(0xffffffff,0xffffffff),(0xffffffff,1),(0x80000000,2),(0x12345678,0x9abcdef0),(0xff,0xff),(0xffff,0xffff)]
v+=[(random.getrandbits(32),random.getrandbits(32)) for _ in range(56)]
print("static const unsigned long A[64]={%s};"%",".join("0x%xUL"%a for a,b in v))
print("static const unsigned long B[64]={%s};"%",".join("0x%xUL"%b for a,b in v))
print("static const unsigned long W[64]={%s};"%",".join("0x%xUL"%((a*b)&0xffffffff) for a,b in v))
P
cat > $T/m.c <<'C'
#include "vec.h"
#ifdef BASE
#define MUL(a, b) (a)
#else
#define MUL(a, b) ((a) * (b))
#endif
unsigned long sink;
int main(void) {
  unsigned fails = 0, i;
  unsigned long r;
  for (i = 0; i < 64; i++) { r = MUL(A[i], B[i]); if (r != W[i]) fails++; sink = r; }
  return fails > 255 ? 255 : fails;
}
C
for O in -O -Oi -Osir; do
  for k in base full; do
    d=""; [ $k = base ] && d=-DBASE
    cl65 -t sim6502 $O $d -I$T -m $T/$k.map -o $T/$k.bin $T/m.c 2>$T/err || { cat $T/err; exit 1; }
    set +e; o=$(sim65 -c $T/$k.bin 2>&1); eval "f_$k=\$?"; set -e
    eval "c_$k=\$(echo \"\$o\" | sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' | head -1)"
    eval "s_$k=\$(stat -c %s $T/$k.bin)"
  done
  echo "cc65 $O: +bytes=$(( s_full - s_base )) fails=$f_full cycles=$(( (c_full - c_base) / 64 ))"
done
for O in -Os -O2; do
  for k in base full; do
    d=""; [ $k = base ] && d=-DBASE
    $B/mos-sim-clang $O $d -I$T -o $T/$k $T/m.c 2>$T/err || { cat $T/err; exit 1; }
    set +e; $B/mos-sim --cycles $T/$k 2>$T/$k.out >/dev/null; eval "f_$k=\$?"; set -e
    eval "c_$k=\$(sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' $T/$k.out | head -1)"
    eval "s_$k=\$($B/llvm-size $T/$k.elf | awk 'NR==2{print \$1}')"
  done
  echo "llvm-mos $O: +bytes=$(( s_full - s_base )) fails=$f_full cycles=$(( (c_full - c_base) / 64 ))"
done
# ours (shift-add, llvm_mos_mulsi3.s) in the llvm-mos __mulsi3 ABI: same C harness, same call glue as the llvm-mos row above
for k in base full; do
  d=""; [ $k = base ] && d=-DBASE
  $B/mos-sim-clang -Os $d -I$B/../mos-platform/common/asminc -Wl,--allow-multiple-definition -I$T -o $T/$k llvmmos_mulsi3.s $T/m.c 2>$T/err || { cat $T/err; exit 1; }
  set +e; $B/mos-sim --cycles $T/$k 2>$T/$k.out >/dev/null; eval "f_$k=\$?"; set -e
  eval "c_$k=\$(sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' $T/$k.out | head -1)"
  eval "s_$k=\$($B/llvm-size $T/$k.elf | awk 'NR==2{print \$1}')"
done
echo "ours shift-add through the llvm-mos ABI (-Os harness): routine bytes=$($B/clang --target=mos -I$B/../mos-platform/common/asminc -c llvmmos_mulsi3.s -o $T/o.o && $B/llvm-size $T/o.o | awk 'NR==2{print $1}') (the link delta is not meaningful, mul.cc.obj stays in the image) fails=$f_full cycles=$(( (c_full - c_base) / 64 ))"
rm -r $T
