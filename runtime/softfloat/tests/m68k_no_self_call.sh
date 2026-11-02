#!/bin/sh
# 68000 has no 32-bit multiply, so a C `*` in __mulsi3 compiles to `jsr __mulsi3` (2026-11-02: instcombine folded the
# 16-bit partial products back into a*b, 34 bytes of infinite recursion). Usage: LLRM_CC_DIR=dir tests/m68k_no_self_call.sh
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:?dir with llrm-cc and llrm-m68k-llc}
t=$(mktemp)
for o in O1 O2 Os; do
  LLRM_CC_TARGET=m68k-68000 "timeout 900 $D/llrm-cc" -$o -S -Iinclude src/int32.c -o "$t" 2>/dev/null || continue
  awk '/^[A-Za-z_][A-Za-z0-9_]*:$/ {fn=$0} /jsr[ \t]+__mulsi3/ && fn=="__mulsi3:" {bad=1} END {exit bad}' "$t" || { echo "FAIL -$o: __mulsi3 calls itself"; rm -f "$t"; exit 1; }
done
rm -f "$t"
