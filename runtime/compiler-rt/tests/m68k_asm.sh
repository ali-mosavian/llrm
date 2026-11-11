#!/bin/sh
# Hand-written m68k f32 routines (config/m68k/sf32.s), gcc-built int32.c.
#   tests/m68k_asm.sh run CPU [n] [main.c]   check against host-FPU vectors under qemu-m68k (default main: m68k_main.c)
#     GEN=tests/gen_vectors_edge.c: operands aimed at the rare paths
#   tests/m68k_asm.sh clocks CPU OUT [vectors.h]   one exe per op (m68k_clocks.c -DOP=0..4) in OUT, priced with `cycles --exe`
#   tests/m68k_asm.sh obj CPU OUT.o           the object alone (bytes: m68k-linux-gnu-size / nm -S)
# CPU 68000 (base body), 68020 (--defsym M68020=1) or 68060 (also M68060=1). Env: GCC_M68K (default ~/scratch/gcc-m68k/root).
set -e
cd "$(dirname "$0")/.."
MODE=$1; CPU=$2
R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$R/usr/bin/m68k-linux-gnu
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
DEF=; [ "$CPU" = 68000 ] || DEF="--defsym M68020=1"; [ "$CPU" = 68060 ] && DEF="$DEF --defsym M68060=1"
$B-as -m$CPU $DEF -o $T/sf32.o config/m68k/sf32.s
[ "$MODE" = obj ] && { cp $T/sf32.o "$3"; exit 0; }
CF="-m$CPU -msoft-float -O2 -nostdlib -static -ffreestanding -Iinclude -I$T"
for f in int32; do $B-gcc $CF -c lib/int/$f.c -o $T/$f.o; done
LIB="$T/sf32.o $T/int32.o"
if [ "$MODE" = clocks ]; then
  O=$3; mkdir -p $O
  if [ -n "$4" ]; then cp "$4" $T/vectors.h; else cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen 3000 > $T/vectors.h; fi
  for op in 0 1 2 3 4; do $B-gcc $CF -O1 -DOP=$op tests/m68k_clocks.c $LIB -o $O/asm_${CPU}_op$op; done
  exit 0
fi
N=${3:-3000}; MAIN=${4:-tests/m68k_main.c}
cc -O2 -ffp-contract=off ${GEN:-tests/gen_vectors.c} -o $T/gen && $T/gen $N > $T/vectors.h
$B-gcc $CF -O1 "$MAIN" $LIB -o $T/exe
qemu-m68k -cpu m$CPU $T/exe && echo exit=0 || echo "exit=$?"
