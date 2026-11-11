#!/bin/sh
# usage: LLRM_CC_DIR=<dir> tests/m68k_run_c.sh CPU OPT main.c lib.c [more.c...]   (CPU 68000|68020, OPT O0|O1|O2|Os)
# Compiles every .c but main.c with llrm-cc (-S), assembles with the m68k GNU as, links freestanding with gcc and runs under qemu-m68k.
# Only labels that start with __ or sf are exported: the corpus has static helpers of the same name in several files (from_mag, add_bits).
# main.c supplies _start and exits via trap #0 (d0=1). Env: GCC_M68K (default ~/scratch/gcc-m68k/root), CFLAGS_EXTRA.
set -e
ulimit -v 16777216
CPU=$1; OPT=$2; MAIN=$3; shift 3
D=${LLRM_CC_DIR:?set LLRM_CC_DIR to the dir holding llrm-cc}; R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT; OBJS=""
for f in "$@"; do
  n=$(basename "$f" .c)
  LLRM_CC_TARGET=m68k-$CPU timeout 900 $D/llrm-cc -$OPT -S $CFLAGS_EXTRA "$f" -o $T/$n.s
  awk 'BEGIN{print ".text"} /^(__|sf)[A-Za-z0-9_]*:$/ {l=$0; sub(/:$/,"",l); print ".globl " l} {print}' $T/$n.s > $T/$n.g.s
  $R/usr/bin/m68k-linux-gnu-as -m$CPU -o $T/$n.o $T/$n.g.s; OBJS="$OBJS $T/$n.o"
done
$R/usr/bin/m68k-linux-gnu-gcc -msoft-float -m$CPU -O1 -nostdlib -static -ffreestanding $CFLAGS_EXTRA "$MAIN" $OBJS -o $T/exe 2>&1 | head -20 || true
[ -x $T/exe ] || { echo "link failed (see above)"; exit 2; }
qemu-m68k -cpu ${QEMU_CPU:-m$CPU} $T/exe; echo "exit=$?"
