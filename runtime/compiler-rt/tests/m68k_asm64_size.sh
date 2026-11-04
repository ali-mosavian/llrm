#!/bin/sh
# Text bytes of config/m68k/sf64.s (add, sub, mul, div and their shared tails) per CPU. Env: GCC_M68K (default ~/scratch/gcc-m68k/root).
set -e
cd "$(dirname "$0")/.."
R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$R/usr/bin/m68k-linux-gnu
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
for c in 68000 68020 68060; do
  tests/m68k_asm64.sh obj $c $T/$c.o
  echo "$c: $($B-size $T/$c.o | awk 'NR==2{print $1}') B"
done
