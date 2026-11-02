#!/bin/sh
# usage: tests/m68k_wrapper_size.sh [sf32.c]: each compare entry point is at most 42 bytes at m68k-68020 -Os (llrm-cc).
# Was 46: each wrapper called cmpsf2 and remapped the unordered result. 42 is what llrm-cc makes of a plain call (no tail call, args copied through d0); libgcc is 26.
set -e
ulimit -v 16777216   # one unbounded compiler run must not take the machine
cd "$(dirname "$0")/.."
SRC=${1:-src/sf32.c}; MAX=42
D=${LLRM_CC_DIR:-$HOME/scratch/mfbuild/keep}; R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_CC_TARGET=m68k-68020 timeout 900 $D/llrm-cc -Os -S -Iinclude "$SRC" -o $T/a.s
awk 'BEGIN{print ".text"} /^[A-Za-z_][A-Za-z0-9_]*:$/ {l=$0; sub(/:$/,"",l); print ".globl " l} {print}' $T/a.s > $T/a.g.s
$R/usr/bin/m68k-linux-gnu-as -m68020 -o $T/a.o $T/a.g.s
total=$($R/usr/bin/m68k-linux-gnu-size $T/a.o | awk 'NR==2{print $1}')
$R/usr/bin/m68k-linux-gnu-nm -n $T/a.o | awk -v tot=$total -v max=$MAX '
  { a[NR]=$1; n[NR]=$3 }
  function hex(s,  i,v,c) { v=0; for (i=1;i<=length(s);i++) { c=index("0123456789abcdef",substr(s,i,1))-1; v=v*16+c } return v }
  END { bad=0
    for (i=1;i<=NR;i++) if (n[i] ~ /^__(cmp|eq|ne|lt|le|gt|ge)sf2$/) {
      sz=(i<NR?hex(a[i+1]):tot)-hex(a[i]); printf "%s %d\n", n[i], sz; if (sz>max) bad=1 }
    exit bad }'
