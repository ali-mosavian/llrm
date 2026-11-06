#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64] tests/f64_float_split_test.sh
# Before: f64 float.c held all four int-to-float entries and archive members link whole, so a program converting only an int32 (fcmp_all) linked __floatdidf, __floatundidf,
# __floatunsidf and sf64_pack too: +484 B at aarch64 -O2 against compiler-rt's one-entry-per-file 60 B. Control: without SF_CONV_SPLIT the other entries are still linked.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0
for t in ld.lld $C; do command -v $t > /dev/null || { echo "FAIL: $t not found"; exit 1; }; done
printf '#undef SF_CONV_SPLIT\n' > $T/nosplit.h
printf 'double __floatsidf(int);\nvoid _start(void) { volatile double d = __floatsidf(3); for (;;) {} }\n' > $T/p.c
for spec in "aarch64-linux-gnu|-mabi=aapcs-soft -mgeneral-regs-only|aarch64" "riscv64-linux-gnu|-march=rv64imac -mabi=lp64|riscv64"; do
  IFS='|' read tc fl cf <<EOT
$spec
EOT
  for v in split nosplit; do
    inc=; [ $v = nosplit ] && inc="-include $T/nosplit.h"; rm -f "${T:?}"/*.o "${T:?}"/l.a
    for f in $(sh tests/fp_sources.sh f64) lib/fp/f32/pack.c; do
      $C --target=$tc $fl -ffp-contract=off -O2 -w -ffreestanding -include stdint.h -include config/$cf/config.h $inc -Iinclude -c $f -o $T/$(echo $f | tr / _).o || { echo "FAIL: compile $f"; exit 2; }
    done
    $C --target=$tc $fl -O2 -w -ffreestanding -c $T/p.c -o $T/p0.o && llvm-ar rcs $T/l.a $T/lib_*.o 2>/dev/null || ar rcs $T/l.a $T/lib_*.o
    ld.lld -static -e _start -o $T/x $T/p0.o $T/l.a 2> $T/err || { echo "FAIL: link $tc $v: $(cat $T/err)"; bad=1; continue; }
    n=$(nm $T/x | grep -c -E ' __float(un)?didf$| __floatunsidf$')
    if [ $v = split ] && [ "$n" -ne 0 ]; then echo "FAIL: $tc a program using only __floatsidf links $n other int-to-float entries"; bad=1; fi
    if [ $v = nosplit ] && [ "$n" -eq 0 ]; then echo "FAIL: $tc control: unsplit float.c no longer links the other entries (test measures nothing)"; bad=1; fi
  done
done
[ $bad = 0 ] && echo "f64_float_split ok"; exit $bad
