#!/bin/bash
# usage: LLRM_Z80_LLC=<llrm-z80-llc> tests/z80_lib.sh OUTDIR [OPT=Os] -> the soft-float and integer library as Z80 .s/.rel in OUTDIR, for run.py's Z80_LIBC_EXTRA.
# C (lib/fp, lib/int) -> the pin's clang for msp430 (16-bit int, 32-bit long) -> llrm-z80-llc -> sdasz80, globals named as the backend's libcalls call them (__adddf3).
# Exits 1 if a symbol the Z80 program sweep links is not defined by the set. -Os: the -O2 backend inlines the compare core into every entry (f64 cmp 10591 B against 2188 B).
set -e
cd "$(dirname "$0")/.."
OUT=${1:?output directory}; OPT=${2:-Os}; LLC=${LLRM_Z80_LLC:?path of llrm-z80-llc}; CLANG=${LLRM_PIN_CLANG:-/opt/llrm-pin/bin/clang}
DL="e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
MODULES="fp/f64/add_sub fp/f64/mul fp/f64/div fp/f64/float_si fp/f64/float_usi fp/f64/float_di fp/f64/float_udi fp/f64/pack fp/f32/pack fp/f32/extend fp/f32/trunc
 fp/f32/float_di fp/f32/fix_di fp/f64/fix_si fp/f64/fix_usi fp/f64/fix_di fp/f64/fix_udi
 fp/f64/cmp_cmp fp/f64/cmp_eq fp/f64/cmp_ne fp/f64/cmp_lt fp/f64/cmp_le fp/f64/cmp_ge fp/f64/cmp_gt fp/f32/neg fp/f64/neg fp/f64/unord int/clzsi2"
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
sdasz80 -g -o "$OUT/umulhisi3.rel" config/z80/umulhisi3.s; cp config/z80/umulhisi3.s "$OUT/umulhisi3.s"
# f32 add sub mul div, int<->float and compares are hand asm (config/z80/sf32_*.s): 1705 + 171 B for all of them, 3-14x SDCC's clocks. Like SDCC's own f32 library they keep
# operands in static cells and are not reentrant, and they clobber IY (free in the llrm-z80 convention).
# one member per operation (sdld pulls a whole member: a one-op program links under 1000 B, tests/z80_sf32_split_test.sh); the add/sub member keeps the name fp_f32_add_sub the z80 A/B instrument requires
for pair in sf32_common:fp_f32_common sf32_round:fp_f32_round sf32_addsub:fp_f32_add_sub sf32_muldiv:fp_f32_mul_div sf32_float:fp_f32_float sf32_fix:fp_f32_fix sf32_cmp:fp_f32_cmp; do src=${pair%%:*}; dst=${pair##*:}; cp config/z80/$src.s "$OUT/$dst.s"; sdasz80 -g -o "$OUT/$dst.rel" "$OUT/$dst.s"; done
bash "$(dirname "$0")/z80_lib_symbols.sh" "$OUT" || exit 1
bash "$(dirname "$0")/z80_lib_closed.sh" "$OUT" || exit 1
echo "ok: $(grep -hE '^[A-Za-z_][A-Za-z0-9_]*::|\.globl' $OUT/*.s | wc -l) globals in $OUT"
