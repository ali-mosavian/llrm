#!/bin/bash
# Reached bytes of __addsf3 and __divsf3 (sf32.c + sf32_conv.c, SF_INTABI, m32 header) at gcc -m32 -Os must stay under their caps.
# The special-case preamble once tested both operands before the swap: add 516 B; Newton division instead of hardware divides: div 562 B.
set -e
cd "$(dirname "$0")/.."; T=$(mktemp -d); trap 'rm -r $T' EXIT
F="-Os -ffreestanding -fno-pic -fno-builtin -w -Iinclude -DSF_INTABI=1 -include stdint.h -include include/sf_x86_m32.h"
gcc -m32 $F -c src/sf32.c -o $T/a.o; gcc -m32 $F -c src/sf32_conv.c -o $T/c.o
echo 'void _start(void){}' > $T/s.c; gcc -m32 -c $T/s.c -o $T/s.o
ld -m elf_i386 -e _start -o $T/o.elf $T/a.o $T/c.o $T/s.o 2>/dev/null
for rc in __addsf3:470 __divsf3:505; do
  n=$(python3 tests/reach32.py $T/o.elf ${rc%:*} | cut -d' ' -f1)
  [ "$n" -le "${rc#*:}" ] || { echo "FAIL ${rc%:*} reaches $n B > ${rc#*:}"; exit 1; }
  echo "ok: ${rc%:*} reaches $n B (cap ${rc#*:})"
done
