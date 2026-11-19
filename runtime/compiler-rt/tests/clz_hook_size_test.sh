#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64] tests/clz_hook_size_test.sh -> config/{aarch64,riscv64} map SF_CLZ32/SF_CLZ64 to the native clz.
# Without them the denormal normalise inlines two ~50-insn shift ladders: aarch64 -Os f64 mul 760 B, div 820, float 500 against 316, 384, 296 with them (riscv64 614, 658, 394 against 264, 308, 244).
# The bound must hold with the config and be exceeded with its SF_CLZ lines removed, so the check cannot pass on a missing hook.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0
size_of() { # triple config file strip
  cfg=config/$2/config.h; [ "$4" = strip ] && { grep -v 'SF_CLZ' $cfg > $T/c.h; cfg=$T/c.h; }
  $C --target=$1 -Os -w -ffreestanding -include stdint.h -include $cfg -Iinclude -c lib/fp/$3.c -o $T/a.o || { echo 0; return; }
  size -A $T/a.o | awk '$1==".text"{t+=$2} END{print t+0}'; }
check() { # triple config file bound
  w=$(size_of $1 $2 $3 keep); n=$(size_of $1 $2 $3 strip)
  [ "$w" -gt 0 ] && [ "$w" -le "$4" ] || { echo "FAIL: $1 $3 with the clz hooks is $w B (bound $4)"; bad=1; }
  [ "$n" -gt "$4" ] || { echo "FAIL: $1 $3 without the clz hooks is $n B, not above $4: the bound does not discriminate"; bad=1; }; }
for f in f64/mul f64/div f64/float; do
  case $f in f64/mul) a=340; r=290;; f64/div) a=410; r=330;; f64/float) a=320; r=270;; esac
  check aarch64-linux-gnu aarch64 $f $a
  check riscv64-linux-gnu riscv64 $f $r
done
[ $bad = 0 ] && echo "clz_hook_size ok"; exit $bad
