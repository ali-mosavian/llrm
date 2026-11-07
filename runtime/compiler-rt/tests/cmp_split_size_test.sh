#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64] tests/cmp_split_size_test.sh
# Before: aarch64 and riscv64 inlined the compare core into each of the seven entries of a width: a program using the six comparisons of f32 and f64 linked 1064 B of
# compare code at aarch64 -O2 against compiler-rt's 432 (fcmp_all +632 B). With SF_CMP_SPLIT the entries alias one core. Bound 480; control: without it the bound is exceeded.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0
for t in ld.lld $C; do command -v $t > /dev/null || { echo "FAIL: $t not found"; exit 1; }; done
printf '#undef SF_CMP_SPLIT\n' > $T/nosplit.h
{ for w in d s; do for e in eq ne lt le ge gt; do printf 'int __%s%sf2(%s, %s);\n' $e $w $( [ $w = d ] && echo "double double" || echo "float float" ); done; done
  printf 'volatile int r;\nvoid _start(void) {\n'
  for w in d s; do for e in eq ne lt le ge gt; do printf '  r += __%s%sf2(1, 2);\n' $e $w; done; done; printf '  for (;;) {}\n}\n'; } > $T/p.c
for spec in "aarch64-linux-gnu|-mabi=aapcs-soft -mgeneral-regs-only|aarch64" "riscv64-linux-gnu|-march=rv64imac -mabi=lp64|riscv64"; do
  IFS='|' read tc fl cf <<EOT
$spec
EOT
  for v in split nosplit; do
    inc=; [ $v = nosplit ] && inc="-include $T/nosplit.h"; rm -f "${T:?}"/lib_*.o "${T:?}"/l.a
    for f in lib/fp/f64/cmp*.c lib/fp/f32/cmp*.c; do
      $C --target=$tc $fl -ffp-contract=off -O2 -w -ffreestanding -include stdint.h -include config/$cf/config.h $inc -Iinclude -c $f -o $T/lib_$(echo $f | tr / _).o || { echo "FAIL: compile $f"; exit 2; }
    done
    $C --target=$tc $fl -O2 -w -ffreestanding -c $T/p.c -o $T/p0.o && ar rcs $T/l.a $T/lib_*.o
    ld.lld -static -e _start -o $T/x $T/p0.o $T/l.a 2> $T/err || { echo "FAIL: link $tc $v: $(cat $T/err)"; bad=1; continue; }
    n=$(size -A $T/x | awk '$1 ~ /^\.text/{t+=$2}END{print t}'); n=$((n - $(size -A $T/p0.o | awk '$1 ~ /^\.text/{t+=$2}END{print t}')))
    if [ $v = split ] && [ "$n" -gt 480 ]; then echo "FAIL: $tc six comparisons of both widths link $n B of compare code (bound 480)"; bad=1; fi
    if [ $v = nosplit ] && [ "$n" -le 480 ]; then echo "FAIL: $tc control: unsplit compare is $n B, under the bound (test measures nothing)"; bad=1; fi
  done
done
[ $bad = 0 ] && echo "cmp_split_size ok"; exit $bad
