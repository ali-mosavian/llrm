#!/bin/bash
# usage: LLRM_Z80_LLC=<llrm-z80-llc> [CFG=0] tests/z80/f64_mul_clk.sh -> f64 multiply on ucsim_z80 over 32 operands: the xor of the results must match the host FPU
# and the mean clocks per call stay under LIMIT. Without config/z80/config.h (CFG=0) every 32x32 is four __mulsi3 (65237 clk/call) and the test fails.
set -e
cd "$(dirname "$0")/../.."; R=$PWD; LLC=${LLRM_Z80_LLC:?path of llrm-z80-llc}; P=${LLRM_PIN_CLANG:-/opt/llrm-pin/bin/clang}; LIMIT=${LIMIT:-50000}
T=$(mktemp -d); trap 'rm -r $T' EXIT; cd $T
DL="e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"; INC="-include $R/config/z80/config.h"; [ "${CFG-1}" = 0 ] && INC=
cc() { # src name defs
  $P --target=msp430 -O2 -Xclang -disable-llvm-passes -ffreestanding -I$R/include $INC $3 -S -emit-llvm $1 -o $2.raw.ll
  sed -E "s|^target datalayout = .*|target datalayout = \"$DL\"|; /^target triple/d" $2.raw.ll > $2.ll; $LLC -S -Os -o $2.s $2.ll; sed -i 's/___/__/g' $2.s; sdasz80 -g -o $2.rel $2.s; }
mkdir lib; for m in fp/f64/mul fp/f64/pack fp/f64/unpack int/clzsi2; do cc $R/lib/$m.c lib/$(basename $m) ""; done
cp $R/../../crates/target/llrm-z80/runtime/z80_rt.s . ; sdasz80 -g -o lib/zrt.rel z80_rt.s; sdasz80 -g -o lib/int32.rel $R/config/z80/int32.s; sdasz80 -g -o lib/umulhisi3.rel $R/config/z80/umulhisi3.s
(INC=; cc $R/lib/fp/f32/pack.c lib/f32pack "") ; ls lib/*.rel | xargs -n1 basename | sed 's/\.rel$//' | sort -r > lib/sf.lib   # zrt first: its shifts, not the per-bit ones of lib/int
cc $R/tests/z80/f64_drv.c drv '-DOP=__muldf3'
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > crt0.s; sdasz80 -o crt0.rel crt0.s
printf 'typedef unsigned long long u64;\nu64 __muldf3(u64 a, u64 b) { return a ^ b; }\n' > stub.c; (INC=; cc stub.c stub "")
sdldz80 -i -b _CODE=0x100 -b _DATA=0x4000 r.ihx crt0.rel drv.rel -k $T/lib -l sf 2>&1 | grep -i "undefined" && { echo "FAIL: unresolved symbol"; exit 1; }
sdldz80 -i -b _CODE=0x100 -b _DATA=0x4000 b.ihx crt0.rel drv.rel stub.rel -k $T/lib -l sf 2>&1 | grep -i "undefined" || true
cat > host.c <<'H'
#include <stdio.h>
#include <string.h>
typedef unsigned long long u64;
int main(void){ u64 s=0x9E3779B97F4A7C15ull,am=0; for(int i=0;i<32;i++){u64 v[2];for(int k=0;k<2;k++){s^=s<<13;s^=s>>7;s^=s<<17;v[k]=(s&0x800FFFFFFFFFFFFFull)|((u64)(1010+((s>>52)&0x1F))<<52);}
 double a,b,m;u64 um;memcpy(&a,&v[0],8);memcpy(&b,&v[1],8);m=a*b;memcpy(&um,&m,8);am^=um;} printf("%016llx\n",am);}
H
cc -O0 -o host host.c 2>/dev/null || gcc -O0 -o host host.c; want=$(./host)
run() { printf 'run\nstate\ndump rom 0x8f00 0x8f07\nquit\n' | timeout 900 ucsim_z80 -t Z80 -q -w $1.ihx -c - 2>&1; }
o=$(run r); c=$(echo "$o" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1); b=$(run b | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
got=$(echo "$o" | sed -n 's/^0x8f00 *\(.*\) [^ ]*$/\1/p' | head -1 | awk '{s="";for(i=NF;i>=1;i--)s=s $i;print s}')
clk=$(( (c - b) / 32 )); echo "f64 mul: $clk clk/call, xor $got (host $want)"
[ "$got" = "$want" ] || { echo "FAIL: result differs from the host"; exit 1; }
[ $clk -lt $LIMIT ] || { echo "FAIL: $clk clk/call, limit $LIMIT"; exit 1; }
