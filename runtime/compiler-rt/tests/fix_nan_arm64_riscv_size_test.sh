#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64] tests/fix_nan_arm64_riscv_size_test.sh -> .text of f64 fix_si.c at -Os, aarch64 and riscv64.
# The configs lacked SF_FIX_NAN_UNDEF, so each carried a NaN test returning 0 that compiler-rt does not have: aarch64 fix_si 104 B against 80.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0
for spec in "aarch64-linux-gnu|-mabi=aapcs-soft -mgeneral-regs-only|aarch64|80" "riscv64-linux-gnu|-march=rv64imac -mabi=lp64|riscv64|80"; do
  IFS='|' read tc fl cf lim <<INNER
$spec
INNER
  $C --target=$tc $fl -Os -w -ffreestanding -include stdint.h -include config/$cf/config.h -Iinclude -c lib/fp/f64/fix_si.c -o $T/o.o || exit 2
  n=$(size -A $T/o.o | awk '$1==".text"{print $2}')
  [ "$n" -le "$lim" ] || { echo "FAIL: $tc f64 fix_si.c is $n B (bound $lim)"; bad=1; }
done
[ $bad = 0 ] && echo "fix_nan_arm64_riscv_size ok"; exit $bad
