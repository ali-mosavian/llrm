#!/bin/sh
# usage: bench_cc65.sh -> uint32 a*b through cc65's tosumuleax slot: cc65's own lmul and ours (cc65_tosumuleax.s). The 472 vectors of bench_variants.sh in one
# run (mean = (full - copy build) / 472, glue in both), the worst pair 0xffffffff*0xffffffff in a run of its own; bytes = lmul.o CODE from the ld65 map / our
# object's CODE. sim65 has no clock register, so the worst case is a separate one-call run, not a maximum over a set.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d)
python3 - $T <<'P'
import random,sys
random.seed(7)
v=[(0,0),(1,1),(0xffffffff,0xffffffff),(0xffffffff,1),(0x80000000,2),(0x12345678,0x9abcdef0),(0xff,0xff),(0xffff,0xffff)]
v+=[(random.getrandbits(32),random.getrandbits(32)) for _ in range(56)]
v+=[(0xffffffff,0xffffffff),(1,0xffffffff),(0xffffffff,0),(0x80000000,0x80000000),(0x01010101,0x01010101),(0xff00ff00,0x00ff00ff),(0xffffff,0xffffff),(3,0x7fffffff)]
for _ in range(400): v.append((random.getrandbits(random.randint(1,32)),random.getrandbits(random.randint(1,32))))
def w(name,v):
    f=open(sys.argv[1]+'/'+name,'w'); f.write("#define NV %d\n"%len(v))
    for n,k in (("A",0),("B",1)): f.write("static const unsigned long %s[NV]={%s};\n"%(n,",".join("0x%xUL"%t[k] for t in v)))
    f.write("static const unsigned long W[NV]={%s};\n"%",".join("0x%xUL"%((a*b)&0xffffffff) for a,b in v))
w('vec.h',v); w('vec1.h',[(0xffffffff,0xffffffff)])
P
cat > $T/m.c <<'C'
#ifdef ONE
#include "vec1.h"
#else
#include "vec.h"
#endif
#ifdef BASE
#define MUL(a, b) (a)
#else
#define MUL(a, b) ((a) * (b))
#endif
unsigned long sink;
int main(void) {
  unsigned fails = 0, i;
  unsigned long r;
  for (i = 0; i < NV; i++) { r = MUL(A[i], B[i]); if (r != W[i]) fails++; sink = r; }
  return fails > 255 ? 255 : fails;
}
C
ca65 -t sim6502 cc65_tosumuleax.s -o $T/ours.o
od65 --dump-segsize $T/ours.o | awk '/CODE/{print "  ours object CODE bytes:", $2}'
cyc() { # label extra-objects
  for s in "" -DONE; do for k in base full; do d=""; [ $k = base ] && d=-DBASE
    cl65 -t sim6502 -Os $d $s -I$T -m $T/map -o $T/x.bin $T/m.c $2 2>$T/err || { cat $T/err; exit 1; }
    set +e; o=$(sim65 -c $T/x.bin 2>&1); f=$?; set -e
    c=$(echo "$o" | sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' | head -1)
    eval "c_$k$([ -n "$s" ] && echo 1)=$c"; [ $k = full ] && eval "f$([ -n "$s" ] && echo 1)=$f"
  done; done
  echo "$1: mean=$(( (c_full - c_base) / 472 )) worst=$(( c_full1 - c_base1 )) fails=$f/$f1"
}
cyc "cc65 lmul" ""
grep -A1 'lmul.o' $T/map | head -2 | tail -1 | sed 's/^/  /'
cyc "ours v2y" "$T/ours.o"
rm -r $T
