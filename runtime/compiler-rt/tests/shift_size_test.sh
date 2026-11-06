#!/bin/sh
# usage: LLRM_CC_DIR=<dir with llrm-cc> tests/shift_size_test.sh
# Before: __ashldi3/__lshrdi3/__ashrdi3 guarded counts of 64 or more (a contract libgcc and compiler-rt do not have): 84/84/92 B at m68k -Os (linked 106/106/114 against libgcc's 78-86).
# Without the guard 62/62/66 B. Bound 70.
cd "$(dirname "$0")/.." || exit 2; D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; bad=0
for c in 68000 68020; do for f in ashldi3 lshrdi3 ashrdi3; do
  LLRM_CC_TARGET=m68k-$c $D/llrm-cc -Os -c -Iinclude -Ilib/int -include config/m68k/config.h lib/int/$f.c -o $T/o.o || exit 2
  n=$(size -A $T/o.o | awk '$1==".text"{print $2}')
  [ "$n" -le 70 ] || { echo "FAIL: m68k-$c $f.c is $n B (bound 70)"; bad=1; }
done; done
[ $bad = 0 ] && echo "shift_size ok"; exit $bad
