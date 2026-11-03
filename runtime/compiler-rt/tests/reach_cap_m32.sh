#!/bin/bash
# usage: LLRM_CC_DIR=dir tests/reach_cap_m32.sh -> reached bytes of __addsf3 and __divsf3 (sf32.c + sf32_conv.c, SF_INTABI, m32 header, x86_32-linux i486 -Os)
# built by llrm-cc must stay under their caps. The gcc -m32 numbers are printed for information only; no guard follows a competitor's compile.
# History: add 516 B while the special-case preamble tested both operands before the swap; div 557 B with the Newton division instead of hardware divides.
set -e
cd "$(dirname "$0")/.."; D=${LLRM_CC_DIR:?set LLRM_CC_DIR to the dir holding llrm-cc}; T=$(mktemp -d); trap 'rm -r $T' EXIT
F="-Iinclude -DSF_INTABI=1 -include stdint.h -include config/x86_m32/config.h"
echo 'void _start(void){}' > $T/s.c; gcc -m32 -c $T/s.c -o $T/s.o
SRC=$(tests/fp_sources.sh f32); L=; G=
for f in $SRC; do b=$(basename $f .c); LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -Os -c $F $f -o $T/l_$b.o; L="$L $T/l_$b.o"
  gcc -m32 -Os -ffreestanding -fno-pic -fno-builtin -w $F -c $f -o $T/g_$b.o; G="$G $T/g_$b.o"; done
ld -m elf_i386 -e _start -o $T/l.elf $L $T/s.o
ld -m elf_i386 -e _start -o $T/g.elf $G $T/s.o
bad=0
for rc in __addsf3:450 __divsf3:480; do
  n=$(python3 tests/reach32.py $T/l.elf ${rc%:*} | cut -d' ' -f1); g=$(python3 tests/reach32.py $T/g.elf ${rc%:*} | cut -d' ' -f1)
  echo "${rc%:*}: llrm $n B (cap ${rc#*:}); gcc-built $g B (information)"
  [ "$n" -le "${rc#*:}" ] || bad=1
done
[ $bad = 0 ] && echo ok || { echo "FAIL: a reached size is over its cap"; exit 1; }
