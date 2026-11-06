#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64] tests/f64_div_insns_test.sh -> executed instructions per f64 divide, -O2, qemu-user TB trace (deterministic), driver cost subtracted.
# Also size: the three Newton steps unrolled made div.c 544 / 736 B (aarch64 float_conversions_loop 1188 B against compiler-rt's 1156); a loop is 496 / 676. Bound 520 / 700.
# Before: aarch64/riscv64 divided one quotient bit per step: 643 / 693 instructions per call against compiler-rt's 98 / 127 (Newton reciprocal, wide multiply). Bound 200.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0; H=tests/icount
for t in qemu-aarch64 qemu-riscv64 ld.lld $C; do command -v $t > /dev/null || { echo "FAIL: $t not found"; exit 1; }; done
run() { # triple flags qemu cfg nv lib-objects...
  tc=$1; fl=$2; q=$3; cf=$4; nv=$5; shift 5
  $C --target=$tc $fl -O2 -w -ffreestanding -DKIND=1 -DNV=$nv -DLIBFN=$LF -c $H/drv.c -o $T/drv.o && ld.lld -static -e _start -o $T/x.elf $T/drv.o "$@" || return 1
  python3 $H/count.py $q $T/x.elf; }
for spec in "aarch64-linux-gnu|-mabi=aapcs-soft -mgeneral-regs-only -ffp-contract=off|qemu-aarch64|aarch64" "riscv64-linux-gnu|-march=rv64imac -mabi=lp64 -ffp-contract=off|qemu-riscv64|riscv64"; do
  IFS='|' read tc fl q cf <<EOF
$spec
EOF
  for f in div pack; do $C --target=$tc $fl -O2 -w -ffreestanding -include stdint.h -include config/$cf/config.h -Iinclude -c lib/fp/f64/$f.c -o $T/o_$f.o || exit 2; done
  sz=$(size -A $T/o_div.o | awk '$1==".text"{print $2}'); lim=520; [ $cf = riscv64 ] && lim=700
  [ "$sz" -le $lim ] || { echo "FAIL: $tc f64 div.c is $sz B (bound $lim)"; bad=1; }
  $C --target=$tc $fl -O2 -w -ffreestanding -c $H/nul.c -o $T/nul.o
  LF=nul1;     b0=$(run $tc "$fl" $q $cf 0 $T/nul.o) && bn=$(run $tc "$fl" $q $cf 200 $T/nul.o) || { echo "FAIL: $tc null run"; bad=1; continue; }
  LF=__divdf3; o0=$(run $tc "$fl" $q $cf 0 $T/o_div.o $T/o_pack.o) && on=$(run $tc "$fl" $q $cf 200 $T/o_div.o $T/o_pack.o) || { echo "FAIL: $tc div run"; bad=1; continue; }
  n=$(awk -v a=$on -v b=$o0 -v c=$bn -v d=$b0 'BEGIN{printf "%d", ((a-b)-(c-d))/200}')
  [ "$n" -le 200 ] && [ "$n" -gt 20 ] || { echo "FAIL: $tc f64 div executes $n instructions per call (bound 200, floor 20)"; bad=1; }
done
[ $bad = 0 ] && echo "f64_div_insns ok"; exit $bad
