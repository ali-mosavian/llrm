#!/bin/sh
# usage: tests/shift64_6502_test.sh -> the 6502 64-bit shifts are asm members, each assembled alone under 110 B and exporting its routine, and
# fix_di.c / fix_udi.c each define one routine. A program that shifted a u64 by a variable pulled 205 to 230 B of C per routine (double_to_uint64
# -O1 1,851 B, llvm-mos 1,061), and a float to u64 conversion pulled the signed one too (543 B for 206).
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT; bad=0
for r in ashldi3 lshrdi3 ashrdi3; do
  grep -qx "config/6502/$r.s" config/6502/members.txt || { echo "FAIL: $r.s is not in members.txt"; bad=1; continue; }
  ca65 -t sim6502 -I config/6502 -o $T/$r.o config/6502/$r.s || { echo "FAIL: $r.s does not assemble"; bad=1; continue; }
  n=$(od65 --dump-segsize $T/$r.o | awk '/CODE/{print $2; exit}')
  [ "${n:-999}" -le 110 ] || { echo "FAIL: $r.s is $n B (bound 110)"; bad=1; }
  od65 --dump-exports $T/$r.o | grep -q "__$r" || { echo "FAIL: $r.s does not export __$r"; bad=1; }
done
[ -f lib/fp/f32/fix_udi.c ] && ! grep -q fixunssfdi lib/fp/f32/fix_di.c && ! grep -q 'fixsfdi' lib/fp/f32/fix_udi.c || { echo "FAIL: fixsfdi and fixunssfdi share a file"; bad=1; }
[ $bad = 0 ] && echo "shift64_6502 ok"; exit $bad
