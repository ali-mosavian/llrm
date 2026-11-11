#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64] tests/float_di_size_test.sh -> .text of f64 and f32 float_di.c at -O2 with the aarch64/riscv64 configs.
# Before: `x < 0 ? from_mag(SIGN, -x) : from_mag(0, x)` let the compiler copy the 64-bit normaliser into both arms: riscv64 f64 434 B (compiler-rt's floatdidf 270), f32 624 B;
# aarch64 216 / 276. One body on the sign mask: 248 / 438 and 124 / 204.
# f32 then rounds in place (no sf32_pack call): the file grows (f32 aarch64 204 -> 236) and the program shrinks (floatdisf+floatundisf+pack 312 -> 236 at -O2).
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0
command -v $C > /dev/null || { echo "FAIL: $C not found"; exit 1; }
for spec in "aarch64-linux-gnu|-mabi=aapcs-soft -mgeneral-regs-only|aarch64|150 240" "riscv64-linux-gnu|-march=rv64imac -mabi=lp64|riscv64|280 480"; do
  IFS='|' read tc fl cf lim <<INNER
$spec
INNER
  set -- $lim; l64=$1; l32=$2
  for f in f64 f32; do
    $C --target=$tc $fl -O2 -w -ffreestanding -include stdint.h -include config/$cf/config.h -Iinclude -c lib/fp/$f/float_di.c -o $T/o.o || exit 2
    n=$(size -A $T/o.o | awk '$1==".text"{print $2}'); l=$l64; [ $f = f32 ] && l=$l32
    # f32 rounds in place where there is a 64-bit clz (no pack call): floatdisf linked alone was 40 + 84 + 108 (pack) B, 172 for compiler-rt
    [ $f != f32 ] || ! nm $T/o.o | grep -q ' U sf32_pack' || { echo "FAIL: $tc f32 float_di.c calls sf32_pack"; bad=1; }
    [ "$n" -le "$l" ] || { echo "FAIL: $tc $f float_di.c is $n B (bound $l)"; bad=1; }
  done
done
[ $bad = 0 ] && echo "float_di_size ok"; exit $bad
