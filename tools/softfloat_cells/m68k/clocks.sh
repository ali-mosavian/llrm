#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc and cycles> [LIBGCC_OBJS=/tmp/sfl] tools/softfloat_cells/m68k/clocks.sh [OPT=Os] [OUT=dir] -> one executable per (CPU, library, op) priced with `cycles --exe`:
# ours (lib/fp + lib/int by llrm-cc, add/sub/mul/div = config/m68k/sf64.s) and libgcc (lb1sf68.S + fpgnulib objects). 3000 vectors; clk/call = total / 3000; `*` = a data-dependent term at its bound.
# Control: the null op (OP=99, no library call) must price at 0 clocks.
set -e
W=$(cd "$(dirname "$0")/../../../runtime/compiler-rt" && pwd); H=$(cd "$(dirname "$0")" && pwd); cd $W; opt=${1:-Os}; O=${2:-$HOME/scratch/handoff/m68k-clocks64-$opt}; mkdir -p $O
D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; G=${LIBGCC_OBJS:-/tmp/sfl}; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT
export LD_LIBRARY_PATH=$HOME/scratch/gcc-m68k/root/usr/lib/x86_64-linux-gnu; export GCC_M68K=$HOME/scratch/gcc-m68k/root; B=$GCC_M68K/usr/bin/m68k-linux-gnu
cc -O2 -ffp-contract=off tests/gen_vectors64.c -o $T/gen && $T/gen 3000 > $T/vectors64.h
LS=${LIBGCC_SRC:-$HOME/scratch/sf-libgcc-src/libgcc}
for c in 68000 68020; do
  objs=; for src in $(tests/fp_sources.sh f64 f32_64 f32 int); do f=$(basename $(dirname $src))_$(basename $src .c)
    case $src in lib/int/udivdi3.c|lib/int/umoddi3.c|lib/int/divdi3.c|lib/int/moddi3.c|lib/int/udivmod_w.c) [ $c = 68000 ] || continue;; esac # 68020: the divu.l members below
    case $src in lib/fp/f64/add_sub.c|lib/fp/f64/mul.c|lib/fp/f64/div.c) continue;; esac
    LLRM_CC_TARGET=m68k-$c $D/llrm-cc -$opt -S -Iinclude -include config/m68k/config.h ${EXTRA_INC:+-include $EXTRA_INC} $src -o $T/$f.s 2> /dev/null || continue
    awk 'BEGIN{print ".text"} /^(__|sf[0-9]+_)[A-Za-z0-9_]*:$/ {l=$0; sub(/:$/,"",l); print ".globl " l} {print}' $T/$f.s > $T/$f.g.s
    $B-as -m$c -o $T/${f}_$c.o $T/$f.g.s 2> /dev/null && objs="$objs $T/${f}_$c.o"
  done
  if [ $c = 68020 ]; then LLRM_CC=$D/llrm-cc tools/m68k_div64_members.sh 68020 $T/m$c; objs="$objs $T/m$c/udivdi3.o $T/m$c/umoddi3.o $T/m$c/divdi3.o $T/m$c/moddi3.o"; fi # what libsf.a ships
  ds=; [ $c = 68020 ] && ds="--defsym M68020=1"; $B-as -m$c $ds -o $T/sf64_$c.o config/m68k/sf64.s; objs="$objs $T/sf64_$c.o"
  gobjs=; for o in $G/${c}_double.o $G/${c}_floatex.o $G/${c}_fpg_$opt.o; do [ -f $o ] && gobjs="$gobjs $o"; done
  for m in ltdf2 cmpdf2; do $B-gcc -msoft-float -m$c -DL_$m -I$LS/config/m68k -I$LS -c $LS/config/m68k/lb1sf68.S -o $T/g_${m}_$c.o 2> /dev/null && gobjs="$gobjs $T/g_${m}_$c.o"; done
  ints=; [ $c = 68000 ] && ints="$G/${c}_divsi3.o $G/${c}_modsi3.o $G/${c}_udivsi3.o $G/${c}_umodsi3.o $G/${c}_mulsi3.o"
  for op in 0 1 2 3 4 5 6 7 8 9 99; do
    $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -DOP=$op -I$T $H/clocks.c $objs -o $O/ours_${c}_op$op 2> $T/err || { [ $op = 99 ] || echo "build FAIL ours $c op$op: $(grep -m2 -E "undefined|multiple|error" $T/err | tr "\n" " ")"; }
    $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -DOP=$op -I$T $H/clocks.c $gobjs $ints -o $O/libgcc_${c}_op$op 2> $T/err || { [ $op = 99 ] || echo "build FAIL libgcc $c op$op: $(grep -m2 -E "undefined|multiple|error" $T/err | tr "\n" " ")"; }
  done
done
$D/cycles --exe $(ls $O/ours_* $O/libgcc_* 2> /dev/null | sort) > $O/RESULT.md
cat $O/RESULT.md; python3 $H/table.py $O/RESULT.md > $O/TABLE.md; cat $O/TABLE.md
