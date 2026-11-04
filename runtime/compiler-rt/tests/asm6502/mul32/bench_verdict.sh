#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [MOS_RT=mos_rt.s] bench_verdict.sh -> the verdict rows of the 6502 mul32 column: one C program (uint32 a*b over the 472 vectors of
# bench_variants.sh), each compiler with its own call shape: llrm-cc 6502 (mos_rt.s's __mulsi3, and with llrm_mulsi3.s patched in), cc65 -Os, llvm-mos -Os (-Os and -O2).
# bytes = whole-image delta against a plain-copy build of the same program; mean = (full - copy) / 472 cycles; worst = the one-call run of 0xffffffff*0xffffffff
# (full - copy). sim65 has no clock register, so worst is a run of its own for every compiler. Fails are counted by the program itself.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); B=${LLVM_MOS:-$HOME/scratch/fetch-llvm-mos-sdk/x/llvm-mos}/bin
LLRM_CC=${LLRM_CC:-$HOME/targets/gs-out/rbase/llrm-cc}; MOS_RT=${MOS_RT:-$HOME/scratch/handoff/mos_rt.s}
python3 - $T <<'P'
import random,sys
random.seed(7)
v=[(0,0),(1,1),(0xffffffff,0xffffffff),(0xffffffff,1),(0x80000000,2),(0x12345678,0x9abcdef0),(0xff,0xff),(0xffff,0xffff)]
v+=[(random.getrandbits(32),random.getrandbits(32)) for _ in range(56)]
v+=[(0xffffffff,0xffffffff),(1,0xffffffff),(0xffffffff,0),(0x80000000,0x80000000),(0x01010101,0x01010101),(0xff00ff00,0x00ff00ff),(0xffffff,0xffffff),(3,0x7fffffff)]
for _ in range(400): v.append((random.getrandbits(random.randint(1,32)),random.getrandbits(random.randint(1,32))))
def w(name,v):
    f=open(sys.argv[1]+'/'+name,'w'); f.write("#define NV %d\n"%len(v))
    for n,k in (("A",0),("B",1)): f.write("unsigned long %s[NV]={%s};\n"%(n,",".join("0x%xUL"%t[k] for t in v)))
    f.write("unsigned long W[NV]={%s};\n"%",".join("0x%xUL"%((a*b)&0xffffffff) for a,b in v))
w('vec.h',v); w('vec1.h',[(0xffffffff,0xffffffff)])
P
cat > $T/m.c <<'C'
#ifdef ONE
#include "vec1.h"
#else
#include "vec.h"
#endif
#ifdef BASE
#define MUL(a, b) (sinkb = (b), (a))
#else
#define MUL(a, b) ((a) * (b))
#endif
unsigned long sink;
volatile unsigned long sinkb;
unsigned long *volatile pa = A, *volatile pb = B;
int main(void) {
  unsigned fails = 0, i;
  unsigned long r;
  for (i = 0; i < NV; i++) { r = MUL(pa[i], pb[i]); if (r != W[i]) fails++; sink = r; }
  return fails > 255 ? 255 : fails;
}
C
# the runtime with ours in place of its __mulsi3
python3 - $MOS_RT $T <<'P'
import sys
s=open(sys.argv[1]).read(); a=s.index('__mulsi3:'); a=s.index('\n',a)+1; b=s.index('  jmp copy_st_to_sa',a)+len('  jmp copy_st_to_sa\n')
body=open('llrm_mulsi3.s').read().split('__mulsi3:\n',1)[-1]
open(sys.argv[2]+'/rt_ours.s','w').write(s[:a]+body+s[b:])
open(sys.argv[2]+'/rt_stub.s','w').write(s[:a]+'  rts\n'+s[b:])
P
row() { echo "$1: +bytes=$2 mean=$(( ($3 - $4) / 472 )) worst=$(( $5 - $6 )) fails=$7"; }
# llrm-cc: $1 label, $2 runtime file (the base build links rt_stub.s: the routine replaced by rts, so the routine's bytes show in the delta)
llrm() {
  for s in "" -DONE; do for k in base full; do d=""; [ $k = base ] && d=-DBASE
    $LLRM_CC --target=6502-sim65 -Os -S $d $s -I$T $T/m.c -o $T/m.s 2>$T/err || { cat $T/err; exit 1; }
    { grep -h '^rc[0-9]* = ' $T/m.s | sort -u; printf '.export _main\n_main:\n  jsr main\n  rts\n'; grep -v '^rc[0-9]* = ' $T/m.s; if [ $k = base ]; then cat $T/rt_stub.s; else cat $2; fi; } > $T/p.s
    cl65 -t sim6502 -o $T/p.bin $T/p.s 2>$T/err || { cat $T/err; exit 1; }
    set +e; o=$(sim65 -c -x 500000000 $T/p.bin 2>&1); f=$?; set -e
    c=$(echo "$o" | sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' | head -1); sz=$(stat -c %s $T/p.bin)
    eval "c_$k$([ -n "$s" ] && echo 1)=$c; z_$k$([ -n "$s" ] && echo 1)=$sz"; [ $k = full ] && eval "f$([ -n "$s" ] && echo 1)=$f"
  done; done
  row "$1" $(( z_full - z_base )) $c_full $c_base $c_full1 $c_base1 "$f/$f1"
}
llrm "llrm-cc 6502, mos_rt.s __mulsi3" $MOS_RT
llrm "llrm-cc 6502, ours (llrm_mulsi3.s)" $T/rt_ours.s
for O in -Os; do
  for s in "" -DONE; do for k in base full; do d=""; [ $k = base ] && d=-DBASE
    cl65 -t sim6502 $O $d $s -I$T -o $T/x.bin $T/m.c 2>$T/err || { cat $T/err; exit 1; }
    set +e; o=$(sim65 -c $T/x.bin 2>&1); f=$?; set -e
    c=$(echo "$o" | sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' | head -1); sz=$(stat -c %s $T/x.bin)
    eval "c_$k$([ -n "$s" ] && echo 1)=$c; z_$k$([ -n "$s" ] && echo 1)=$sz"; [ $k = full ] && eval "f$([ -n "$s" ] && echo 1)=$f"
  done; done
  row "cc65 $O" $(( z_full - z_base )) $c_full $c_base $c_full1 $c_base1 "$f/$f1"
done
for O in -Os -O2; do
  for s in "" -DONE; do for k in base full; do d=""; [ $k = base ] && d=-DBASE
    $B/mos-sim-clang $O $d $s -I$T -o $T/x $T/m.c 2>$T/err || { cat $T/err; exit 1; }
    set +e; $B/mos-sim --cycles $T/x 2>$T/x.out >/dev/null; f=$?; set -e
    c=$(sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' $T/x.out | head -1); sz=$($B/llvm-size $T/x.elf | awk 'NR==2{print $1}')
    eval "c_$k$([ -n "$s" ] && echo 1)=$c; z_$k$([ -n "$s" ] && echo 1)=$sz"; [ $k = full ] && eval "f$([ -n "$s" ] && echo 1)=$f"
  done; done
  row "llvm-mos $O" $(( z_full - z_base )) $c_full $c_base $c_full1 $c_base1 "$f/$f1"
done
rm -r $T
