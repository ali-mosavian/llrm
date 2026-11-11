#!/bin/sh
# Text bytes of config/m68k/sf32.s per CPU: the 13 symbols libgcc's soft-float also has (add sub mul div neg cmp eq ne lt le gt ge unord),
# and the four int conversions. Env: GCC_M68K (default ~/scratch/gcc-m68k/root).
set -e
cd "$(dirname "$0")/.."
R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$R/usr/bin/m68k-linux-gnu
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
for c in 68000 68020; do
  tests/m68k_asm.sh obj $c $T/$c.o
  total=$($B-size $T/$c.o | awk 'NR==2{print $1}')
  conv=0; for h in $($B-nm -S $T/$c.o | awk '/ T __(float|fix)/ {print $2}'); do conv=$((conv + 0x$h)); done
  echo "$c: 13 symbols $((total - conv)) B, conversions $conv B, total $total B"
done
