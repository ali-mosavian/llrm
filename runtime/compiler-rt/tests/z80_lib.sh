#!/bin/bash
# usage: LLRM_Z80_LLC=<llrm-z80-llc> tests/z80_lib.sh OUTDIR [OPT=Os] -> the soft-float and integer library as Z80 .s/.rel in OUTDIR, for run.py's Z80_LIBC_EXTRA.
# C (lib/fp, lib/int) -> the pin's clang for msp430 (16-bit int, 32-bit long) -> llrm-z80-llc -> sdasz80, globals named as the backend's libcalls call them (__adddf3).
# Exits 1 if a symbol the Z80 program sweep links is not defined by the set. -Os: the -O2 backend inlines the compare core into every entry (f64 cmp 10591 B against 2188 B).
set -e
cd "$(dirname "$0")/.."
OUT=${1:?output directory}; OPT=${2:-Os}; LLC=${LLRM_Z80_LLC:?path of llrm-z80-llc}; CLANG=${LLRM_PIN_CLANG:-/opt/llrm-pin/bin/clang}
DL="e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
MODULES="fp/f64/add_sub fp/f64/mul fp/f64/div fp/f64/float fp/f64/pack fp/f32/pack fp/f32/extend fp/f32/trunc
 fp/f32/float_di fp/f32/fix_di fp/f64/fix_si fp/f64/fix_usi fp/f64/fix_di fp/f64/fix_udi
 fp/f64/cmp fp/f32/neg fp/f64/neg fp/f64/unord int/clzsi2"
mkdir -p "$OUT"; T=$(mktemp -d); trap 'rm -r $T' EXIT
for m in $MODULES; do
  n=$(echo "$m" | tr / _)
  $CLANG --target=msp430 -O2 -Xclang -disable-llvm-passes -ffreestanding -Iinclude -include config/z80/config.h -S -emit-llvm lib/$m.c -o $T/$n.raw.ll
  sed -E "s|^target datalayout = .*|target datalayout = \"$DL\"|; /^target triple/d" $T/$n.raw.ll > $T/$n.ll
  $LLC -S -$OPT -o $OUT/$n.s $T/$n.ll
  sed -i 's/___/__/g' $OUT/$n.s
  sdasz80 -g -o $OUT/$n.rel $OUT/$n.s
done
sdasz80 -g -o "$OUT/div64.rel" config/z80/div64.s; cp config/z80/div64.s "$OUT/div64.s"
# f32 add sub mul div, int<->float and compares are hand asm (config/z80/sf32_arith.s, sf32_cmp.s): 1705 + 171 B for all of them, 3-14x SDCC's clocks. Like SDCC's own f32 library they keep
# operands in static cells and are not reentrant, and they clobber IY (free in the llrm-z80 convention).
# member names kept (fp_f32_add_sub, fp_f32_cmp): the z80 A/B instrument requires fp_f32_add_sub.s and links members by symbol
for pair in sf32_arith:fp_f32_add_sub sf32_cmp:fp_f32_cmp; do src=${pair%%:*}; dst=${pair##*:}; cp config/z80/$src.s "$OUT/$dst.s"; sdasz80 -g -o "$OUT/$dst.rel" "$OUT/$dst.s"; done
defined=$(grep -h '\.globl' $OUT/*.s | awk '{print $2}' | sort -u)
missing=""
for s in __adddf3 __subdf3 __muldf3 __divdf3 __eqdf2 __nedf2 __ltdf2 __ledf2 __gtdf2 __gedf2 __cmpdf2 __addsf3 __subsf3 __mulsf3 __divsf3 __eqsf2 __nesf2 __ltsf2 __lesf2 \
         __gtsf2 __gesf2 __floatsidf __floatunsidf __floatdidf __floatundidf __floatsisf __floatunsisf __fixdfsi __fixunsdfsi __fixdfdi __fixunsdfdi \
         __fixsfsi __fixunssfsi __fixunssfdi __extendsfdf2 __truncdfsf2; do
  echo "$defined" | grep -qx "$s" || missing="$missing $s"
done
[ -z "$missing" ] || { echo "FAIL: not defined:$missing"; exit 1; }
echo "ok: $(echo "$defined" | wc -l) globals in $OUT"
