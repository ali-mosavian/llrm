#!/bin/sh
# Hand-written m68k f64 routines (src/m68k/sf64.s) over gcc-built sf64.c, int64.c and int32.c: sf64.c's add/sub/mul/div are
# weakened, so whatever sf64.s defines is what runs.
#   tests/m68k_asm64.sh run CPU [n] [main.c]   check against host-FPU vectors under qemu-m68k (default main: m68k_main64.c)
#     GEN=tests/gen_vectors64_edge.c: operands aimed at the rare paths
#   tests/m68k_asm64.sh clocks CPU OUT [vectors64.h]   one exe per op (m68k_clocks.c -DF64 -DOP=0..3) in OUT, priced with `cycles --exe`
#   tests/m68k_asm64.sh obj CPU OUT.o          the object alone
# CPU 68000 (base body), 68020 (--defsym M68020=1) or 68060 (also M68060=1). Env: GCC_M68K (default ~/scratch/gcc-m68k/root).
set -e
ulimit -v 16777216
cd "$(dirname "$0")/.."
MODE=$1; CPU=$2
R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$R/usr/bin/m68k-linux-gnu
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
DEF=; [ "$CPU" = 68000 ] || DEF="--defsym M68020=1"; [ "$CPU" = 68060 ] && DEF="$DEF --defsym M68060=1"
$B-as -m$CPU $DEF -o $T/sf64s.o src/m68k/sf64.s
# qemu runs what the CPU would trap on: the 68060 has no 64-bit mul/div, the 68000 no 32-bit mul/div or bitfields
BAN=; [ "$CPU" = 68060 ] && BAN='(mul|div)[us]l [^ ]*,%d[0-7],%d'; [ "$CPU" = 68000 ] && BAN='(mul|div)[us]l|bf'
[ -z "$BAN" ] || ! $B-objdump -d $T/sf64s.o | grep -E "^ *[0-9a-f]+:	[0-9a-f ]+	($BAN)" || { echo "$CPU: instruction it lacks"; exit 1; }
[ "$MODE" = obj ] && { cp $T/sf64s.o "$3"; exit 0; }
CF="-m$CPU -msoft-float -O2 -nostdlib -static -ffreestanding -Iinclude -I$T"
for f in sf64 int64 int32; do timeout 900 $B-gcc $CF -c src/$f.c -o $T/$f.o; done
$B-objcopy -W __adddf3 -W __subdf3 -W __muldf3 -W __divdf3 $T/sf64.o
LIB="$T/sf64s.o $T/sf64.o $T/int64.o $T/int32.o"
if [ "$MODE" = clocks ]; then
  O=$3; mkdir -p $O
  if [ -n "$4" ]; then cp "$4" $T/vectors64.h; else timeout 900 cc -O2 -ffp-contract=off tests/gen_vectors64.c -o $T/gen && $T/gen 3000 > $T/vectors64.h; fi
  for op in 0 1 2 3; do timeout 900 $B-gcc $CF -O1 -DF64 -DOP=$op tests/m68k_clocks.c $LIB -o $O/asm64_${CPU}_op$op; done
  exit 0
fi
N=${3:-3000}; MAIN=${4:-tests/m68k_main64.c}
timeout 900 cc -O2 -ffp-contract=off ${GEN:-tests/gen_vectors64.c} -o $T/gen && $T/gen $N > $T/vectors64.h
timeout 900 $B-gcc $CF -O1 "$MAIN" $LIB -o $T/exe
qemu-m68k -cpu m$CPU $T/exe && echo exit=0 || echo "exit=$?"
