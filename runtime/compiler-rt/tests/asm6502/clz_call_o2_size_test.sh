#!/bin/sh
# usage: LLRM_CC=<llrm-cc> tests/asm6502/clz_call_o2_size_test.sh -> asm lines of f64 mul.c and div.c through llrm-cc 6502-sim65.
# -O2 open-coded the static-inline clz helpers (sf_clz32/64) in every operation: mul.c 2135 lines against 1733 at -Os (5739 B against llvm-mos 5509), div.c 1923 against 1438.
# config/6502 calls the shared __clzsi2 at -O2: 1642 / 1469. -Os is unchanged (1733 / 1438), checked as a ceiling.
cd "$(dirname "$0")/../.." || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; bad=0
for spec in "mul|O2|1700" "div|O2|1550" "mul|Os|1733" "div|Os|1438"; do
  IFS='|' read op lvl lim <<EOF
$spec
EOF
  ${LLRM_CC:?llrm-cc} --target=6502-sim65 -$lvl -DSF_INTABI -DSF_MUL32X32_6502 -include config/6502/config.h -Iinclude -S lib/fp/f64/$op.c -o $T/o.s || exit 2
  n=$(wc -l < $T/o.s)
  [ "$n" -le "$lim" ] || { echo "FAIL: f64 $op.c -$lvl is $n asm lines (bound $lim)"; bad=1; }
done
[ $bad = 0 ] && echo "clz_call_o2_size ok"; exit $bad
