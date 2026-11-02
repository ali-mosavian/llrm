#!/bin/bash
# usage: tests/size_x86.sh [OPT] -> text bytes at x86_32-linux (llrm-cc, -march=i486): ours (sf32.c + sf32_conv.c, 17 symbols), compiler-rt (llrm-cc, the same
# symbols it defines), libgcc soft-fp (host gcc -m32 -march=i486, GCC 15.2 libgcc/soft-fp + config/i386, extracted by the f32 m68k column). Library bytes only.
set -e
ulimit -v 16777216   # one unbounded compiler run must not take the machine
cd "$(dirname "$0")/.."
OPT=${1:-Os}; D=${LLRM_CC_DIR:-$HOME/targets/gs-out/rbase}; CRT=${COMPILER_RT:-$HOME/scratch/llvm-src-m16/compiler-rt/lib/builtins}
S=${LIBGCC_SRC:-$HOME/scratch/sf-libgcc-src/libgcc}; T=$(mktemp -d); trap 'rm -r $T' EXIT
cc1() { LLRM_CC_TARGET=x86_32-linux timeout 900 $D/llrm-cc -march=i486 -$OPT -c "$@" 2>$T/err || { echo "  compile failed: $(head -c 100 $T/err)"; return 1; }; }
t() { size "$1" | awk 'NR==2{print $1}'; }
echo "== x86_32-linux -march=i486 -$OPT, text bytes"
for f in sf32 sf32_conv; do cc1 -Iinclude src/$f.c -o $T/$f.o && echo "ours $f $(t $T/$f.o)"; done
echo "ours exports: $(nm -g --defined-only $T/sf32.o $T/sf32_conv.o | awk 'NF==3{printf "%s ", $3}')"
tot=0; for n in addsf3 subsf3 mulsf3 divsf3 negsf2 fixsfsi fixunssfsi floatsisf floatunsisf comparesf2; do
  if cc1 -I$CRT $CRT/$n.c -o $T/c_$n.o; then s=$(t $T/c_$n.o); echo "compiler-rt $n $s"; fi; done
for n in addsf3 subsf3 mulsf3 divsf3 negsf2 eqsf2 gesf2 lesf2 unordsf2 fixsfsi fixunssfsi floatsisf floatunsisf; do
  gcc -m32 -march=i486 -$OPT -fno-pic -fno-stack-protector -fcf-protection=none -fno-asynchronous-unwind-tables -I$S/soft-fp -I$S/config/i386 -I$S -I$S/../include -c $S/soft-fp/$n.c -o $T/g_$n.o 2>/dev/null && echo "libgcc-softfp $n $(t $T/g_$n.o)"; done
