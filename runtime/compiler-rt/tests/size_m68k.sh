#!/bin/sh
# usage: tests/size_m68k.sh [CPU] [OPT]: text bytes of our f32 routines vs compiler-rt's, both compiled by llrm-cc for m68k.
# Per object, padding free (assembled with gas, `size` text). Env as m68k-run-c.sh (LLRM_CC_DIR, GCC_M68K).
set -e
cd "$(dirname "$0")/.."
CPU=${1:-68000}; OPT=${2:-Os}
D=${LLRM_CC_DIR:-$HOME/scratch/mfbuild/keep}; R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}
CRT=${COMPILER_RT:-$HOME/scratch/llvm-src-m16/compiler-rt/lib/builtins}
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d)
bytes() {   # file, extra flags
  n=$(basename "$1" .c)
  LLRM_CC_TARGET=m68k-$CPU $D/llrm-cc -$OPT -S $2 "$1" -o $T/$n.s 2>$T/$n.err || { echo "$n: compile failed: $(head -c 120 $T/$n.err)"; return; }
  awk 'BEGIN{print ".text"} {print}' $T/$n.s > $T/$n.g.s
  $R/usr/bin/m68k-linux-gnu-as -m$CPU -o $T/$n.o $T/$n.g.s
  printf '%s %s\n' "$n" "$($R/usr/bin/m68k-linux-gnu-size $T/$n.o | awk 'NR==2{print $1}')"
}
echo "== ours (include/, SF_INTABI off), m68k-$CPU -$OPT"
for f in $(tests/fp_sources.sh f32); do bytes $f "-Iinclude"; done
echo "== compiler-rt"
for f in addsf3 subsf3 mulsf3 divsf3 floatsisf fixsfsi comparesf2; do bytes $CRT/$f.c "-I$CRT"; done
rm -r $T
