#!/bin/sh
# usage: bench_variants.sh -> uint32 a*b through llvm-mos's __mulsi3 slot: llvm-mos's own routine and ours (v1 shift-add, v2, v3 from gen_variants.py).
# 472 vectors: the 64 of bench.sh, 8 edge pairs, 400 with operand widths drawn from 1..32 bits. Per call mean and worst cycles (clock read around each call,
# minus the same loop with a plain copy), routine bytes (llvm-mos: address span of __mulsi3 in the linked ELF; ours: .o text), fails vs a*b.
set -e
cd "$(dirname "$0")"; python3 gen_variants.py; T=$(mktemp -d); B=${LLVM_MOS:-$HOME/scratch/fetch-llvm-mos-sdk/x/llvm-mos}/bin; INC=$B/../mos-platform/common/asminc
python3 - > $T/vec.h <<'P'
import random
random.seed(7)
v=[(0,0),(1,1),(0xffffffff,0xffffffff),(0xffffffff,1),(0x80000000,2),(0x12345678,0x9abcdef0),(0xff,0xff),(0xffff,0xffff)]
v+=[(random.getrandbits(32),random.getrandbits(32)) for _ in range(56)]
v+=[(0xffffffff,0xffffffff),(1,0xffffffff),(0xffffffff,0),(0x80000000,0x80000000),(0x01010101,0x01010101),(0xff00ff00,0x00ff00ff),(0xffffff,0xffffff),(3,0x7fffffff)]
for _ in range(400): v.append((random.getrandbits(random.randint(1,32)),random.getrandbits(random.randint(1,32))))
print("#define NV %d"%len(v))
for n,k in (("A",0),("B",1)): print("static const unsigned long %s[NV]={%s};"%(n,",".join("0x%xUL"%t[k] for t in v)))
print("static const unsigned long W[NV]={%s};"%",".join("0x%xUL"%((a*b)&0xffffffff) for a,b in v))
P
cat > $T/m.c <<'C'
#include <stdio.h>
#include "vec.h"
#ifdef BASE
#define MUL(a, b) (a)
#else
#define MUL(a, b) ((a) * (b))
#endif
static volatile unsigned long *const clk = (volatile unsigned long *)0xFFF0;
int main(void) {
  unsigned long sum = 0, mx = 0, t0, t1, d, r; unsigned fails = 0, i;
  for (i = 0; i < NV; i++) {
    t0 = *clk; r = MUL(A[i], B[i]); t1 = *clk; d = t1 - t0; sum += d; if (d > mx) mx = d;
    if (r != W[i]) fails++;
  }
  printf("%lu %lu %u\n", sum, mx, fails); return 0;
}
C
run() { # name extra-sources
  for k in base full; do d=""; [ $k = base ] && d=-DBASE
    $B/mos-sim-clang -Os $d -I$T -I$INC -Wl,--allow-multiple-definition -o $T/$k $2 $T/m.c 2>$T/err || { cat $T/err; exit 1; }
    set +e; $B/mos-sim $T/$k > $T/$k.out 2>/dev/null; set -e
  done
  set -- $1 $(cat $T/base.out) $(cat $T/full.out)
  echo "$1: mean=$(( ($5 - $2) / 472 )) worst=$(( $6 - $3 )) fails=$7 (per-call glue included in both)"
}
run "llvm-mos" ""
$B/mos-sim-clang -Os -I$T -o $T/th $T/m.c
python3 - $T/th.elf $B <<'PY'
import subprocess,sys
o=subprocess.run([sys.argv[2]+'/llvm-nm','-n',sys.argv[1]],capture_output=True,text=True).stdout.split('\n')
s=[(int(l.split()[0],16),l.split()[2]) for l in o if len(l.split())==3]
i=[k for k,(a,n) in enumerate(s) if n=='__mulsi3'][0]
j=i+1
while s[j][0]==s[i][0]: j+=1
print('  llvm-mos routine bytes (address span of __mulsi3 to the next symbol):', s[j][0]-s[i][0])
PY
for v in "" _v2 _v2y _v2b _v3; do
  n="ours$v"; [ -z "$v" ] && n="ours_v1"
  run "$n" llvmmos_mulsi3$v.s
  $B/clang --target=mos -I$INC -c llvmmos_mulsi3$v.s -o $T/o.o && echo "  routine bytes: $($B/llvm-size $T/o.o | awk 'NR==2{print $1}')"
done
# the shipped file src/6502/mulsi3.S, three flag sets
for f in "" -DSF_MUL_SMALL -DSF_MUL_FAST -DSF_MUL_PORTABLE; do
  run "mulsi3.S${f:+_}${f#-D}" "$f ../../../src/6502/mulsi3.S"
done
rm -r $T
