#!/bin/bash
# usage: LLRM_Z80_LLC=<llrm-z80-llc> [CFG=0] tests/z80/f64_div_clk.sh -> f64 divide on ucsim_z80: 16 edge pairs (1/3, 0, 1/0, overflow, underflow, denormal
# quotients) and 8 random ones; the xor of the quotients must match the host FPU and the mean clocks per call stay under LIMIT. CFG=0 (no config/z80/config.h)
# runs the compiled 57-step loop, ~3.0k clk per step, and fails.
set -e
cd "$(dirname "$0")/../.."; R=$PWD; LLC=${LLRM_Z80_LLC:?path of llrm-z80-llc}; P=${LLRM_PIN_CLANG:-/opt/llrm-pin/bin/clang}; LIMIT=${LIMIT:-60000}
T=$(mktemp -d); trap 'rm -r $T' EXIT; cd $T
DL="e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"; INC="-include $R/config/z80/config.h"; [ "${CFG-1}" = 0 ] && INC=
cc() { # src name
  $P $XF --target=msp430 -O2 -Xclang -disable-llvm-passes -ffreestanding -I$R/include $INC -S -emit-llvm $1 -o $2.raw.ll
  sed -E "s|^target datalayout = .*|target datalayout = \"$DL\"|; /^target triple/d" $2.raw.ll > $2.ll; $LLC -S -Os -o $2.s $2.ll; sed -i 's/___/__/g' $2.s; sdasz80 -g -o $2.rel $2.s; }
mkdir lib; for m in fp/f64/div fp/f64/pack fp/f64/unpack; do cc $R/lib/$m.c lib/$(basename $m); done
cc $R/lib/int/clzsi2.c lib/clzsi2; (INC=; cc $R/lib/int/int64.c lib/int64) 2>/dev/null || true; rm -f lib/int64.*
cp $R/../../crates/target/llrm-z80/runtime/z80_rt.s . ; sdasz80 -g -o lib/zrt.rel z80_rt.s; sdasz80 -g -o lib/int32.rel $R/config/z80/int32.s; sdasz80 -g -o lib/umulhisi3.rel $R/config/z80/umulhisi3.s
sdasz80 -g -o lib/div64.rel $R/config/z80/div64.s
ls lib/*.rel | xargs -n1 basename | sed 's/\.rel$//' | sort -r > lib/sf.lib
XF=$DRVFLAGS cc $R/tests/z80/f64_div_drv.c drv
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > crt0.s; sdasz80 -o crt0.rel crt0.s
printf 'typedef unsigned long long u64;\nu64 __divdf3(u64 a, u64 b) { return a ^ b; }\n' > stub.c; (INC=; cc stub.c stub)
sdldz80 -i -b _CODE=0x100 -b _DATA=0x7000 r.ihx crt0.rel drv.rel -k $T/lib -l sf 2>&1 | grep -i "undefined" && { echo "FAIL: unresolved symbol"; exit 1; }
sdldz80 -i -b _CODE=0x100 -b _DATA=0x7000 b.ihx crt0.rel drv.rel stub.rel -k $T/lib -l sf 2>&1 | grep -i "undefined" || true
gcc -O0 -DHOST $DRVFLAGS -o host $R/tests/z80/f64_div_drv.c; want=$(./host)
run() { printf 'run\nstate\ndump rom 0x8f00 0x8f07\nquit\n' | timeout 1500 ucsim_z80 -t Z80 -q -w $1.ihx -c - 2>&1; }
o=$(run r); c=$(echo "$o" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1); b=$(run b | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
got=$(echo "$o" | sed -n 's/^0x8f00 *\(.*\) [^ ]*$/\1/p' | head -1 | awk '{s="";for(i=NF;i>=1;i--)s=s $i;print s}')
clk=$(( (c - b) / ${NCALL:-24} )); echo "f64 div: $clk clk/call, xor $got (host $want)"
[ "$got" = "$want" ] || { echo "FAIL: result differs from the host"; exit 1; }
[ $clk -lt $LIMIT ] || { echo "FAIL: $clk clk/call, limit $LIMIT"; exit 1; }
