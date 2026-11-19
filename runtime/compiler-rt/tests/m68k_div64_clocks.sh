#!/bin/bash
# usage: LLRM_CC_DIR=<dir with cycles> LIBGCC2_OBJS=<dir from tools/softfloat_cells/m68k/libgcc2.sh> tests/m68k_div64_clocks.sh [OUT]
# Clocks per call of the 64-bit divide libcalls, asm (config/m68k/div64) vs the stock libgcc2 members, 68020/68030/68040, 3000 random vectors (zero divisors skipped).
# The null op (99) must price at the loop alone. * = data-dependent bound.
set -e
cd "$(dirname "$0")/.."; D=${LLRM_CC_DIR:?}; G=${LIBGCC2_OBJS:?}; O=${1:-$(mktemp -d)}; mkdir -p $O
export LD_LIBRARY_PATH=$HOME/scratch/gcc-m68k/root/usr/lib/x86_64-linux-gnu GCC_M68K=$HOME/scratch/gcc-m68k/root; B=$GCC_M68K/usr/bin/m68k-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
cc -O2 tests/gen_div64.c -o $T/gen && $T/gen 3000 > $T/vectors.h
CALLS=$(sed -n '/^{/p' $T/vectors.h | sed -n '401,3400p' | grep -vc '^{0x[0-9a-f]*u,0x[0-9a-f]*u,0x00000000u,0x00000000u,')
for f in udivdi3 umoddi3 divdi3 moddi3; do $B-as -m68020 --defsym M68020=1 -I config/m68k/div64 -o $T/a_$f.o config/m68k/div64/$f.s; done
for op in 0 1 2 3 99; do
  $B-gcc -m68020 -O1 -nostdlib -static -ffreestanding -DOP=$op -I$T -c tests/m68k_div64_clocks.c -o $T/m$op.o
  $B-ld -static -e _start -o $O/asm_68020_op$op $T/m$op.o $T/a_*.o
  $B-ld -static -e _start -o $O/libgcc_68020_op$op $T/m$op.o $(ls $G/68020_{udivdi3,umoddi3,divdi3,moddi3,udivmoddi4}.o) 2> /dev/null || true
done
# signed ops again with half the operands negative (the unsigned fast path of divdi3/moddi3 is then not taken): names *_68020_neg2/3
for op in 2 3; do
  $B-gcc -m68020 -O1 -nostdlib -static -ffreestanding -DOP=$op -DSIGNMIX -I$T -c tests/m68k_div64_clocks.c -o $T/n$op.o
  $B-ld -static -e _start -o $O/asm_68020_neg$op $T/n$op.o $T/a_*.o
  $B-ld -static -e _start -o $O/libgcc_68020_neg$op $T/n$op.o $(ls $G/68020_{udivdi3,umoddi3,divdi3,moddi3,udivmoddi4}.o)
done
CYCLES_CPUS=68020,68030,68040 CYCLES_CALLS=$CALLS $D/cycles --exe $(ls $O/asm_* $O/libgcc_* | sort)
