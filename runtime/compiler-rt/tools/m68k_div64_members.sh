#!/bin/bash
# usage: [LLRM_CC=<llrm-cc>] tools/m68k_div64_members.sh CPU OUTDIR -> OUTDIR/{udivdi3,umoddi3,divdi3,moddi3,muldi3}.o (+ udivmod_w.o, udivmoddi4.o and divmoddi4.o for the C, the pair that `has_div_rem_op(64)` calls on 68000 and 68060): the 64-bit divide and multiply libcalls for one CPU.
# 68020/68030/68040: config/m68k/div64/*.s (divu.l 64/32). 68000/68060: lib/int C, built by llrm-cc (-Os). The libsf.a builder takes these four in place of lib/int's.
# Also OUT/mulsi3.o: config/m68k/mulsi3.s, for every CPU (68000/68010: three mulu.w; 68020 and later: mulu.l).
# Env: M68K_ROOT (default ~/scratch/gcc-m68k/root).
set -e
cd "$(dirname "$0")/.."; R=$PWD; CPU=${1:?cpu}; OUT=${2:?outdir}; ROOT=${M68K_ROOT:-$HOME/scratch/gcc-m68k/root}
export LD_LIBRARY_PATH=$ROOT/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}; AS=$ROOT/usr/bin/m68k-linux-gnu-as
mkdir -p "$OUT"
MULDEF=; case $CPU in 68020|68030|68040|68060) MULDEF="--defsym M68020=1";; esac
"$AS" -m$CPU $MULDEF -o "$OUT/mulsi3.o" "$R/config/m68k/mulsi3.s"
case $CPU in
68020|68030|68040)
  for f in udivdi3 umoddi3 divdi3 moddi3 muldi3; do "$AS" -m$CPU --defsym M68020=1 -I "$R/config/m68k/div64" -o "$OUT/$f.o" "$R/config/m68k/div64/$f.s"; done;;
68000|68060)
  CC=${LLRM_CC:?path of llrm-cc}
  for f in udivdi3 umoddi3 divdi3 moddi3 udivmoddi4 divmoddi4 udivmod_w; do
    LLRM_CC_TARGET=m68k-$CPU "$CC" -Os -S -DSF_INTABI -include "$R/config/m68k/config.h" -I"$R/include" "lib/int/$f.c" -o "$OUT/$f.s"
    "$AS" -m$CPU -o "$OUT/$f.o" "$OUT/$f.s"
  done
  if [ $CPU = 68000 ]; then "$AS" -m$CPU -I "$R/config/m68k/div64" -o "$OUT/muldi3.o" "$R/config/m68k/div64/muldi3.s" # mulu.w asm
  else LLRM_CC_TARGET=m68k-$CPU "$CC" -Os -S -DSF_INTABI -include "$R/config/m68k/config.h" -I"$R/include" lib/int/muldi3.c -o "$OUT/muldi3.s"; "$AS" -m$CPU -o "$OUT/muldi3.o" "$OUT/muldi3.s"; fi;;
*) echo "cpu $CPU?" >&2; exit 1;;
esac
