#!/bin/bash
# usage: LLRM_CC=<llrm-cc> [M68K_ROOT=~/scratch/gcc-m68k/root] tools/build_rt_m68k.sh OUTDIR [CPU=68000] [OPT=Os]
# -> OUTDIR/libsf.a: the soft-float and 64-bit integer runtime of compiler-rt for m68k, one object per source file (the linker takes only the members a program names).
# Link it before libgcc: `m68k-linux-gnu-gcc -m<cpu> -nostdlib -static prog.o rt.o -LOUTDIR -lsf -lgcc` (libgcc's own float routines are the hard-float multilib's).
# The helpers take and return bit patterns (SF_INTABI): doubles and floats are passed as the C ABI passes them, on the stack and in d0:d1 / d0.
set -e
cd "$(dirname "$0")/.."; R=$PWD
OUT=${1:?output directory}; CPU=${2:-68000}; OPT=${3:-Os}; CC=${LLRM_CC:?path of llrm-cc}; ROOT=${M68K_ROOT:-$HOME/scratch/gcc-m68k/root}
export LD_LIBRARY_PATH=$ROOT/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}; AS=$ROOT/usr/bin/m68k-linux-gnu-as; AR=$ROOT/usr/bin/m68k-linux-gnu-ar
mkdir -p "$OUT"; rm -f "$OUT"/*.o "$OUT"/libsf.a
for f in lib/fp/f32/*.c lib/fp/f64/*.c lib/int/int64.c lib/int/clzsi2.c; do
  n=$(echo "${f#lib/}" | tr / _ | sed 's/\.c$//')
  LLRM_CC_TARGET=m68k-$CPU "$CC" -$OPT -S -DSF_INTABI -include "$R/config/m68k/config.h" -I"$R/include" "$f" -o "$OUT/$n.s"
  "$AS" -m$CPU -o "$OUT/$n.o" "$OUT/$n.s"
done
"$AR" rcs "$OUT/libsf.a" "$OUT"/*.o
echo "libsf.a: $(ls "$OUT"/*.o | wc -l) members in $OUT"
