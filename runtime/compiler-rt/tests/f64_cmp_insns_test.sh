#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64] tests/f64_cmp_insns_test.sh -> executed instructions per __ltdf2 and __gedf2 call, -O2, qemu-user TB trace (deterministic), driver cost subtracted.
# Before: the swap-and-select core ran 18.0 / 19.7 instructions on aarch64 / riscv64 against compiler-rt's 16.0 / 15.0 (fcmp_all +9% / +4.7% instructions). Bound 17. __gedf2 on riscv64 ran 16.7 (compiler-rt 14.1) through a negated reversed core: bound 16.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0; H=tests/icount
for t in qemu-aarch64 qemu-riscv64 ld.lld $C; do command -v $t > /dev/null || { echo "FAIL: $t not found"; exit 1; }; done
run() { # flags qemu nv lib-objects...
  fl=$1; q=$2; nv=$3; shift 3
  $C --target=$TC $fl -O2 -w -ffreestanding -DKIND=6 -DNV=$nv -DLIBFN=$LF -c $H/drv.c -o $T/drv.o && ld.lld -static -e _start -o $T/x.elf $T/drv.o "$@" || return 1
  python3 $H/count.py $q $T/x.elf; }
for spec in "aarch64-linux-gnu|-mabi=aapcs-soft -mgeneral-regs-only -ffp-contract=off|qemu-aarch64|aarch64" "riscv64-linux-gnu|-march=rv64imac -mabi=lp64 -ffp-contract=off|qemu-riscv64|riscv64"; do
  IFS='|' read TC fl q cf <<INNER
$spec
INNER
  $C --target=$TC $fl -O2 -w -ffreestanding -c $H/nul.c -o $T/nul.o
  LF=nul6;    b0=$(run "$fl" $q 0 $T/nul.o) && bn=$(run "$fl" $q 200 $T/nul.o) || { echo "FAIL: $TC null run"; bad=1; continue; }
  for ent in "__ltdf2 cmp_cmp 17" "__gedf2 cmp_ge 16"; do
    set -- $ent
    $C --target=$TC $fl -O2 -w -ffreestanding -include stdint.h -include config/$cf/config.h -Iinclude -c lib/fp/f64/$2.c -o $T/o_cmp.o || exit 2
    LF=$1; o0=$(run "$fl" $q 0 $T/o_cmp.o) && on=$(run "$fl" $q 200 $T/o_cmp.o) || { echo "FAIL: $TC $1 run"; bad=1; continue; }
    n=$(awk -v a=$on -v b=$o0 -v c=$bn -v d=$b0 'BEGIN{printf "%.1f", ((a-b)-(c-d))/200}')
    awk -v n=$n -v m=$3 'BEGIN{exit !(n <= m && n > 5)}' || { echo "FAIL: $TC $1 executes $n instructions per call (bound $3)"; bad=1; }
  done
done
[ $bad = 0 ] && echo "f64_cmp_insns ok"; exit $bad
