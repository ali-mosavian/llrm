#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64] tests/f64_float_size_test.sh -> each int-to-binary64 routine, linked alone (gc-sections), stays small at -Os.
# Before: every conversion pulled the generic from_mag and sf64_pack: aarch64 floatsidf 300 B, floatdidf 296. Bounds = compiler-rt's soft path (-D__SOFTFP__; its default aarch64 build
# is the FPU trick, 52 B, not comparable) 64/60/172/152 aarch64, 66/48/150/132 riscv64.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0
for t in ld.lld size $C; do command -v $t > /dev/null || { echo "f64_float_size: $t not found"; exit 2; }; done
check() { # triple config sym bound
  inc="-include stdint.h -include config/$2/config.h"
  for f in float pack; do $C --target=$1 -Os -w -ffreestanding -ffunction-sections $inc "-DSF_NAME(n)=sfb_##n" -Iinclude -c lib/fp/f64/$f.c -o $T/$f.o || exit 2; done
  ld.lld -static --gc-sections -e sfb_$3 -o $T/x $T/float.o $T/pack.o 2> $T/err || { echo "FAIL: link $1 $3: $(cat $T/err)"; bad=1; return; }
  n=$(size -A $T/x | awk '$1 ~ /^\.text/{t+=$2} END{print t+0}')
  [ "$n" -le "$4" ] || { echo "FAIL: $1 $3 links to $n B (bound $4)"; bad=1; }; }
check aarch64-linux-gnu aarch64 floatsidf 70
check aarch64-linux-gnu aarch64 floatunsidf 60
check aarch64-linux-gnu aarch64 floatdidf 172
check aarch64-linux-gnu aarch64 floatundidf 152
check riscv64-linux-gnu riscv64 floatsidf 66
check riscv64-linux-gnu riscv64 floatunsidf 48
check riscv64-linux-gnu riscv64 floatdidf 150
check riscv64-linux-gnu riscv64 floatundidf 132
[ $bad = 0 ] && echo "f64_float_size ok"; exit $bad
