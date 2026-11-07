#!/bin/bash
# __muldi3 on the m68k: config/m68k/div64/muldi3.s (three mulu.l, 68020/030/040) against host products under qemu-m68k; the 68000 and 68060 keep lib/int/muldi3.c.
#   tests/m68k_muldi3.sh [CPU...]   default 68000 68020 68030 68040 68060. LLRM_CC=<llrm-cc> builds the C members. Env: GCC_M68K.
# The 68020 member is 40 B; the C one was 144 (libgcc 52): the bound fails the C build.
set -e
ulimit -v 16777216
cd "$(dirname "$0")/.."
R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$R/usr/bin/m68k-linux-gnu
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
[ $# -gt 0 ] || set -- 68000 68020 68030 68040 68060
cat > $T/gen.c <<'C'
#include <stdint.h>
#include <stdio.h>
int main(void)
{
    uint64_t s = 88172645463325252ull, e[] = {0, 1, 2, 0xffffffffull, 0x100000000ull, 0x7fffffffffffffffull, 0x8000000000000000ull, ~0ull, 0xffff0000ffffull};
    int n = sizeof e / sizeof *e;
    puts("static const unsigned V[][6] = {");
    for (int i = 0; i < 3000; i++) {
        uint64_t a, b;
        s ^= s << 13; s ^= s >> 7; s ^= s << 17; a = i < n * n ? e[i / n] : s;
        s ^= s << 13; s ^= s >> 7; s ^= s << 17; b = i < n * n ? e[i % n] : s;
        uint64_t r = a * b;
        printf("{0x%xu,0x%xu,0x%xu,0x%xu,0x%xu,0x%xu},\n", (unsigned)(a >> 32), (unsigned)a, (unsigned)(b >> 32), (unsigned)b, (unsigned)(r >> 32), (unsigned)r);
    }
    puts("};");
    return 0;
}
C
cc -O2 $T/gen.c -o $T/gen && $T/gen > $T/vectors.h
cat > $T/main.c <<'C'
#include <stdint.h>
#include "vectors.h"
/* the 68000 C member calls it; a 68000 has no 32-bit multiply */
unsigned __mulsi3(unsigned a, unsigned b) { unsigned r = 0; for (; b; b >>= 1, a <<= 1) if (b & 1) r += a; return r; }
/* through a pointer: gcc inlines a 64-bit multiply at -m68020 (mulu.l), and the member under test was never called */
uint64_t __muldi3(uint64_t, uint64_t);
static uint64_t (*volatile mul)(uint64_t, uint64_t) = __muldi3;
#define W(h, l) (((uint64_t)(h) << 32) | (l))
int main(void)
{
    long bad = 0;
    for (unsigned i = 0; i < sizeof V / sizeof *V; i++) {
        volatile uint64_t a = W(V[i][0], V[i][1]), b = W(V[i][2], V[i][3]);
        bad += mul(a, b) != W(V[i][4], V[i][5]);
    }
    __asm__ volatile("move.l %0,%%d1\n\tmoveq #1,%%d0\n\ttrap #0" : : "d"(bad > 255 ? 255 : bad) : "d0", "d1");
    return 0;
}
C
for c in "$@"; do
  mkdir -p $T/$c; LLRM_CC=${LLRM_CC:-} tools/m68k_div64_members.sh $c $T/$c
  $B-gcc -m$c -msoft-float -O1 -nostdlib -static -ffreestanding -I$T -c $T/main.c -o $T/main$c.o
  $B-ld -static -e main -o $T/$c/exe $T/main$c.o $T/$c/muldi3.o
  qemu-m68k -cpu m$c $T/$c/exe && echo "$c: exit=0" || { echo "$c: exit=$?"; exit 1; }
  size=$($B-size $T/$c/muldi3.o | awk 'NR==2{print $1}')
  case $c in 68020|68030|68040)
    [ "$size" -le 40 ] || { echo "FAIL: $c __muldi3 is $size B, bound 40"; exit 1; }
    $B-objdump -d $T/$c/exe | awk '/<__muldi3>:/{f=1;next} /^$/{f=0} f' | grep -qE 'mulul' || { echo "$c: no 64-bit mulu.l"; exit 1;};;
  esac
  # control: a member that returns 0 must fail the same run
  printf '\t.text\n\t.globl __muldi3\n__muldi3:\n\tclr.l %%d0\n\tclr.l %%d1\n\trts\n' > $T/$c/zero.s; $B-as -m$c -o $T/$c/zero.o $T/$c/zero.s
  $B-ld -static -e main -o $T/$c/zexe $T/main$c.o $T/$c/zero.o
  if qemu-m68k -cpu m$c $T/$c/zexe; then echo "$c: control: a __muldi3 returning 0 passed"; exit 1; fi
  echo "$c: __muldi3 $size B"
done
