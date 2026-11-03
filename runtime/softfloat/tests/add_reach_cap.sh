#!/bin/bash
# Reached bytes of __addsf3 (sf32.c + sf32_conv.c, SF_INTABI, m32 header) at gcc -m32 -Os must stay <= CAP.
# The special-case preamble once tested both operands before the swap: 516 B.
set -e
cd "$(dirname "$0")/.."; CAP=${CAP:-470}; T=$(mktemp -d); trap 'rm -r $T' EXIT
F="-Os -ffreestanding -fno-pic -fno-builtin -w -Iinclude -DSF_INTABI=1 -include stdint.h -include include/sf_x86_m32.h"
gcc -m32 $F -c src/sf32.c -o $T/a.o; gcc -m32 $F -c src/sf32_conv.c -o $T/c.o
echo 'void _start(void){}' > $T/s.c; gcc -m32 -c $T/s.c -o $T/s.o
ld -m elf_i386 -e _start -o $T/o.elf $T/a.o $T/c.o $T/s.o 2>/dev/null
n=$(python3 tests/reach32.py $T/o.elf __addsf3 | cut -d' ' -f1)
[ "$n" -le "$CAP" ] || { echo "FAIL __addsf3 reaches $n B > $CAP"; exit 1; }
echo "ok: __addsf3 reaches $n B (cap $CAP)"
