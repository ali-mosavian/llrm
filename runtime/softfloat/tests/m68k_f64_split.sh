#!/bin/bash
# usage: LLRM_CC_DIR=<dir> tests/m68k_f64_split.sh [CPU] [OPT] -> per-function instruction counts of sf64.c on one vector per op and class (qemu trace, symbol column),
# ours via llrm-cc -S with every label exported. Queue: add, mul, div on normal*normal, denormal operand, far exponents; read the share of each function.
set -e
cd "$(dirname "$0")/.."
D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; c=${1:-68020}; opt=${2:-Os}; T=$(mktemp -d); trap 'rm -r $T' EXIT
export LD_LIBRARY_PATH=$HOME/scratch/gcc-m68k/root/usr/lib/x86_64-linux-gnu; B=$HOME/scratch/gcc-m68k/root/usr/bin/m68k-linux-gnu
for f in sf64 sf32 sf32_conv sf32_conv64 int64 int32; do
  LLRM_CC_TARGET=m68k-$c $D/llrm-cc -$opt -S -Iinclude src/$f.c -o $T/$f.s
  awk 'BEGIN{print ".text"} /^(__|sf)[A-Za-z0-9_]*:$/ {l=$0; sub(/:$/,"",l); print ".globl " l} {print}' $T/$f.s > $T/$f.g.s
  $B-as -m$c -o $T/$f.o $T/$f.g.s
done
# name op a_hi a_lo b_hi b_lo: normal*normal, denormal*normal, far exponents (add only)
queue="nn 0 0x40280000 0 0x3FF40000 0
nn 2 0x40280000 0 0x3FF40000 0
nn 3 0x40280000 0 0x3FF40000 0
dn 0 0x00080000 0 0x3FF40000 0
dn 2 0x00080000 0 0x3FF40000 0
far 0 0x40280000 0 0x38000000 0"
echo "$queue" | while read name op ah al bh bl; do
  mkdir -p $T/v; printf '#define NV 1\nstatic const uint32_t V[NV][4] = {{%su,%su,%su,%su}};\n' $ah $al $bh $bl > $T/v/vectors64.h
  $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -DF64 -DOP=$op -I$T/v tests/m68k_clocks.c $T/sf64.o $T/sf32.o $T/sf32_conv.o $T/sf32_conv64.o $T/int64.o $T/int32.o -o $T/x
  qemu-m68k -cpu m$c -one-insn-per-tb -d exec -D $T/log $T/x
  echo "== $name op$op"; $B-nm -n $T/x | awk '$2 ~ /^[tT]$/ {print $1, $3}' > $T/syms
  grep '^Trace' $T/log | sed -E 's/.*\/0*([0-9a-f]+)\/[0-9a-f]+\/[0-9a-f]+\].*/\1/' > $T/pcs
  python3 - $T/syms $T/pcs <<'PY'
import sys, bisect, collections
syms = sorted((int(a, 16), n) for a, n in (l.split() for l in open(sys.argv[1])))
addrs = [a for a, _ in syms]
c = collections.Counter(syms[bisect.bisect_right(addrs, int(pc, 16)) - 1][1] for pc in open(sys.argv[2]))
for n, k in c.most_common(8):
    if n not in ("main", "_start") and not n.startswith("h_"):
        print(f"{k:6d} {n}")
PY
done
