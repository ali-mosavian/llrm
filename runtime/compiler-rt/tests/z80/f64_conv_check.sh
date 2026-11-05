#!/bin/bash
# usage: LLRM_Z80_LLC=<llrm-z80-llc> tests/z80/f64_conv_check.sh -> f64 <-> int32/int64 conversions of lib/fp/f64 on ucsim_z80 against the host FPU (the xor of 4 conversions over 17 integers and 17 doubles).
# A backend miscompile that corrupts library code compiled by it (a hoisted call frame passed stack arguments 6 bytes off in fix_di) changes the fold.
set -e
cd "$(dirname "$0")/../.."; R=$PWD; LLC=${LLRM_Z80_LLC:?path of llrm-z80-llc}; P=${LLRM_PIN_CLANG:-/opt/llrm-pin/bin/clang}
T=$(mktemp -d); trap 'rm -r $T' EXIT; cd $T
DL="e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"; INC="-include $R/config/z80/config.h"
cc() { $P --target=msp430 -O2 -Xclang -disable-llvm-passes -ffreestanding -I$R/include $INC -S -emit-llvm $1 -o $2.raw.ll
  sed -E "s|^target datalayout = .*|target datalayout = \"$DL\"|; /^target triple/d" $2.raw.ll > $2.ll; $LLC -S -Os -o $2.s $2.ll; sed -i 's/___/__/g' $2.s; sdasz80 -g -o $2.rel $2.s; }
mkdir lib; for m in float fix_si fix_usi fix_di fix_udi pack; do cc $R/lib/fp/f64/$m.c lib/f64_$m; done; cc $R/lib/int/clzsi2.c lib/clzsi2
cp $R/../../crates/target/llrm-z80/runtime/z80_rt.s . ; sdasz80 -g -o lib/zrt.rel z80_rt.s; sdasz80 -g -o lib/int32.rel $R/config/z80/int32.s
ls lib/*.rel | xargs -n1 basename | sed 's/\.rel$//' | sort -r > lib/sf.lib
cc $R/tests/z80/f64_conv_drv.c drv
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > crt0.s; sdasz80 -o crt0.rel crt0.s
sdldz80 -i -b _CODE=0x100 -b _DATA=0x7000 r.ihx crt0.rel drv.rel -k $T/lib -l sf 2>&1 | grep -i "undefined" && { echo "FAIL: unresolved symbol"; exit 1; }
gcc -O0 -DHOST -o host $R/tests/z80/f64_conv_drv.c; want=$(./host)
o=$(printf 'run\nstate\ndump rom 0x8f00 0x8f3f\nquit\n' | timeout 900 ucsim_z80 -t Z80 -q -w r.ihx -c - 2>&1)
got=$(echo "$o" | grep -E '^0x8f[0-3][0-9a-f] ' | sed -E 's/^0x8f[0-9a-f]+ +//; s/ +[^ ]{8}$//' | awk '{for(i=1;i<=NF;i++) b[n++]=$i} END{for(k=0;k<8;k++){s="";for(j=7;j>=0;j--) s=s b[k*8+j]; printf "%s%s", s, (k==7?"":" ")}; print ""}')
echo "f64 conversions: xor $got (host $want)"
[ "$got" = "$want" ] || { echo "FAIL: conversions differ from the host"; exit 1; }
