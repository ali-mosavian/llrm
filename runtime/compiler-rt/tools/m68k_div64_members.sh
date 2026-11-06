#!/bin/bash
# usage: [LLRM_CC=<llrm-cc>] tools/m68k_div64_members.sh CPU OUTDIR -> OUTDIR/{udivdi3,umoddi3,divdi3,moddi3}.o (+ udivmod_w.o for the C): the 64-bit divide libcalls for one CPU.
# 68020/68030/68040: config/m68k/div64/*.s (divu.l 64/32). 68000/68060: lib/int C, built by llrm-cc (-Os). The libsf.a builder takes these four in place of lib/int's.
# Env: M68K_ROOT (default ~/scratch/gcc-m68k/root).
set -e
cd "$(dirname "$0")/.."; R=$PWD; CPU=${1:?cpu}; OUT=${2:?outdir}; ROOT=${M68K_ROOT:-$HOME/scratch/gcc-m68k/root}
export LD_LIBRARY_PATH=$ROOT/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}; AS=$ROOT/usr/bin/m68k-linux-gnu-as
mkdir -p "$OUT"
case $CPU in
68020|68030|68040)
  for f in udivdi3 umoddi3 divdi3 moddi3; do "$AS" -m$CPU --defsym M68020=1 -I "$R/config/m68k/div64" -o "$OUT/$f.o" "$R/config/m68k/div64/$f.s"; done;;
68000|68060)
  CC=${LLRM_CC:?path of llrm-cc}
  for f in udivdi3 umoddi3 divdi3 moddi3 udivmod_w; do
    LLRM_CC_TARGET=m68k-$CPU "$CC" -Os -S -DSF_INTABI -include "$R/config/m68k/config.h" -I"$R/include" "lib/int/$f.c" -o "$OUT/$f.s"
    "$AS" -m$CPU -o "$OUT/$f.o" "$OUT/$f.s"
  done;;
*) echo "cpu $CPU?" >&2; exit 1;;
esac
