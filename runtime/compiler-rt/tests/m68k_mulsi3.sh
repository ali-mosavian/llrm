#!/bin/bash
# __mulsi3 (config/m68k/mulsi3.s) at the 68000, 68020, 68040 and 68060: a*b and b*a are both the product, and run the same instructions
# (the shift-add loop it replaced ran over the bits of its second operand: quicksort 68000 -O2 read 1.89 M or 2.61 M clocks for the same sums,
# 313011 against 208359 instructions by the order of a 1000-call loop).
# Env: CYCLES (the cycles binary), GCC_M68K; MULSI3_C=<llrm-cc> builds the old C routine (lib/int/int32.c) in its place.
set -e
cd "$(dirname "$0")/.."
: "${CYCLES:?the cycles binary}"
R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$R/usr/bin/m68k-linux-gnu
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu GCC_M68K=$R
T=$(mktemp -d); trap 'rm -r $T' EXIT
for cpu in 68000 68020 68040 68060; do
  def=; case $cpu in 68000) ;; *) def="--defsym M68020=1";; esac
  if [ -n "$MULSI3_C" ]; then LLRM_CC_TARGET=m68k-$cpu $MULSI3_C -O2 -S -Iinclude lib/int/int32.c -o $T/m.s; $B-as -m$cpu -o $T/m.o $T/m.s
  else $B-as -m$cpu $def -o $T/m.o config/m68k/mulsi3.s; fi
  for width in narrow wide; do
    for order in small_first big_first; do
cat > $T/p.c <<C
#include <stdint.h>
uint32_t __mulsi3(uint32_t, uint32_t);
static uint32_t (*volatile mul)(uint32_t, uint32_t) = __mulsi3;
static uint32_t slow(uint32_t a, uint32_t b) { uint32_t r = 0; for (; b; b >>= 1, a <<= 1) if (b & 1) r += a; return r; }
int main(void)
{
    static const uint32_t edge[] = {0, 1, 2, 0xffff, 0x10000, 0x12345, 0x7fffffff, 0x80000000, 0xffffffff, 0xdeadbeef, 7, 255};
    uint32_t bad = 0;
    for (unsigned i = 0; i < 12; i++) for (unsigned j = 0; j < 12; j++) bad += mul(edge[i], edge[j]) != slow(edge[i], edge[j]);
    for (uint32_t i = 1; i <= 1000; i++) {
        volatile uint32_t small = (i & 15) + 1, big = $( [ $width = narrow ] && echo '0x7fff - i' || echo '0x12345678 + i' );
        bad += mul($( [ $order = small_first ] && echo 'small, big' || echo 'big, small' )) != slow(small, big);
    }
    __asm__ volatile("move.l %0,%%d1\n\tmoveq #1,%%d0\n\ttrap #0" : : "d"(bad > 255 ? 255 : bad) : "d0", "d1");
    return 0;
}
C
      $B-gcc -m$cpu -msoft-float -O1 -nostdlib -static -ffreestanding -c $T/p.c -o $T/p.o
      $B-ld -static -e main -o $T/$width.$order.exe $T/p.o $T/m.o
      qemu-m68k -cpu m$cpu $T/$width.$order.exe || { echo "$cpu $width $order: wrong product (exit $?)"; exit 1; }
      CYCLES_FUNCS=main,__mulsi3 CYCLES_CPUS=$cpu CYCLES_QEMU_CPU=m$cpu $CYCLES --exe $T/$width.$order.exe | awk -F'|' '/\.exe/{gsub(/ /,"",$3); print $3}' > $T/$width.$order.insns
    done
    echo "$cpu $width: small_first $(cat $T/$width.small_first.insns) big_first $(cat $T/$width.big_first.insns) instructions"
    cmp -s $T/$width.small_first.insns $T/$width.big_first.insns || { echo "FAIL: the order changes the instruction count"; exit 1; }
  done
done
echo ok
