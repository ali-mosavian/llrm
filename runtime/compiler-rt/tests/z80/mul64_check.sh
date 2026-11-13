#!/bin/bash
# usage: LLRM_Z80_LLC=<llrm-z80-llc> tests/z80/mul64_check.sh -> config/z80/mul64.s on ucsim_z80 against the host's unsigned __int128 over every pair of edge
# operands (zero, one, all ones, single words, carries that ripple the full 128 bits) and 16 random ones: the rotating xor of the low and of the high halves must match.
set -e
cd "$(dirname "$0")/../.."; R=$PWD; LLC=${LLRM_Z80_LLC:?path of llrm-z80-llc}; P=${LLRM_PIN_CLANG:-/opt/llrm-pin/bin/clang}
T=$(mktemp -d); trap 'rm -r $T' EXIT; cd $T
DL="e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
cat > vec.h <<'H'
static const unsigned long long E[] = {0ull, 1ull, 2ull, 0xFFFFull, 0x10000ull, 0xFFFFFFFFull, 0x100000000ull, 0xFFFFFFFFFFFFull, 0xFFFFFFFFFFFFFFFFull,
  0x8000000000000000ull, 0x7FFFFFFFFFFFFFFFull, 0x0001000100010001ull, 0xFFFF0000FFFF0000ull, 0x00FF00FF00FF00FFull, 0xFFFFFFFF00000001ull};
#define NE ((int)(sizeof E / sizeof E[0]))
H
cat > drv.c <<'H'
#include "vec.h"
typedef unsigned long long u64;
extern u64 sf_m64_a, sf_m64_b, sf_m64_lo, sf_m64_hi;
void __sf_mul64(void);
volatile u64 sink[2];
static u64 rotl1(u64 x) { return x << 1 | x >> 63; }
int main(void) {
    u64 lo = 0, hi = 0, s = 0x9E3779B97F4A7C15ull; int i, j;
    for (i = 0; i < NE + 4; i++) for (j = 0; j < NE + 4; j++) {
        u64 a, b;
        if (i < NE) a = E[i]; else { s ^= s << 13; s ^= s >> 7; s ^= s << 17; a = s; }
        if (j < NE) b = E[j]; else { s ^= s << 13; s ^= s >> 7; s ^= s << 17; b = s; }
        sf_m64_a = a; sf_m64_b = b; __sf_mul64();
        lo = rotl1(lo) ^ sf_m64_lo; hi = rotl1(hi) ^ sf_m64_hi;
    }
    *(volatile u64 *)0x8f00 = lo; *(volatile u64 *)0x8f08 = hi;
    return 0;
}
H
$P --target=msp430 -O2 -Xclang -disable-llvm-passes -ffreestanding -I$R/include -S -emit-llvm drv.c -o drv.raw.ll
sed -E "s|^target datalayout = .*|target datalayout = \"$DL\"|; /^target triple/d" drv.raw.ll > drv.ll; $LLC -S -Os -o drv.s drv.ll; sed -i 's/___/__/g' drv.s; sdasz80 -g -o drv.rel drv.s
cp $R/../../crates/target/llrm-z80/runtime/z80_rt.s .; sdasz80 -g -o zrt.rel z80_rt.s; sdasz80 -g -o mul64.rel $R/config/z80/mul64.s
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > crt0.s; sdasz80 -o crt0.rel crt0.s
out=$(sdldz80 -i -b _CODE=0x100 -b _DATA=0x4000 r.ihx crt0.rel drv.rel mul64.rel zrt.rel 2>&1) && ! grep -qi "undefined" <<<"$out" || { echo "FAIL: link: $out"; exit 1; }
cat > host.c <<'H'
#include <stdio.h>
#include "vec.h"
typedef unsigned long long u64;
static u64 rotl1(u64 x) { return x << 1 | x >> 63; }
int main(void) {
    u64 lo = 0, hi = 0, s = 0x9E3779B97F4A7C15ull;
    for (int i = 0; i < NE + 4; i++) for (int j = 0; j < NE + 4; j++) {
        u64 a, b;
        if (i < NE) a = E[i]; else { s ^= s << 13; s ^= s >> 7; s ^= s << 17; a = s; }
        if (j < NE) b = E[j]; else { s ^= s << 13; s ^= s >> 7; s ^= s << 17; b = s; }
        unsigned __int128 p = (unsigned __int128)a * b;
        lo = rotl1(lo) ^ (u64)p; hi = rotl1(hi) ^ (u64)(p >> 64);
    }
    printf("%016llx%016llx\n", lo, hi);
}
H
gcc -O1 -o host host.c; want=$(./host)
o=$(printf 'run\nstate\ndump rom 0x8f00 0x8f0f\nquit\n' | timeout 900 ucsim_z80 -t Z80 -q -w r.ihx -c - 2>&1)
got=$(echo "$o" | sed -n 's/^0x8f0[08] *\(.*\) [^ ]*$/\1/p' | awk '{for(i=NF;i>=1;i--)printf "%s",$i} END{print ""}')
echo "mul64: $got (host $want)"
[ "$got" = "$want" ] || { echo "FAIL: result differs from the host"; exit 1; }
echo ok
