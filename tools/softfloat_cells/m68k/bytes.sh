#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc> [LIBGCC_OBJS=/tmp/sfl] tools/softfloat_cells/m68k/bytes.sh [OPT=Os] -> per f64 entry, linked .text+.rodata+.data bytes (ENTRIES="sym ..." limits it) of a program that calls only that entry, ours vs libgcc, 68000 and 68020.
# Ours: lib/fp + lib/int files built by llrm-cc into an archive (one object per file, as the runtime links; add/sub/mul/div are config/m68k/sf64.s, hand asm). libgcc: GCC 15.2 lb1sf68.S objects (_double, _floatex) and fpgnulib, given as whole objects
# (/tmp/sfl, built by tests/build_m68k_clocks.sh). Each column links with ld against only its own objects; the null program (no call) is subtracted. Control: calling a symbol that neither column defines must fail to link.
set -e
HH=$(cd "$(dirname "$0")" && pwd)
W=$(cd "$(dirname "$0")/../../../runtime/compiler-rt" && pwd); cd $W; opt=${1:-Os}
D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; G=${LIBGCC_OBJS:-/tmp/sfl}; T=$(mktemp -d); LG2=$T/lg2; $HH/libgcc2.sh $LG2; trap 'rm -r "${T:?}"' EXIT
export LD_LIBRARY_PATH=$HOME/scratch/gcc-m68k/root/usr/lib/x86_64-linux-gnu; B=$HOME/scratch/gcc-m68k/root/usr/bin/m68k-linux-gnu
entries="${ENTRIES:-__muldi3 __udivdi3 __divdi3 __umoddi3 __moddi3 __ashldi3 __lshrdi3 __ashrdi3 __floatdidf __floatundidf __fixdfdi __fixunsdfdi __adddf3 __subdf3 __muldf3 __divdf3 __ltdf2 __gtdf2 __eqdf2 __floatsidf __floatunsidf __fixdfsi __fixunsdfsi __extendsfdf2 __truncdfsf2}"
printf 'int main(void); void *volatile f = main; int main(void) { return f != 0; }\n' > $T/null.c # the same code and data as a probe, so the subtraction leaves only the entry
for c in 68000 68020; do
  mkdir -p $T/$c; ar_objs=
  for src in $(tests/fp_sources.sh f64 f32_64 f32 int); do f=$(basename $(dirname $src))_$(basename $src .c)
    case $src in lib/int/udivdi3.c|lib/int/umoddi3.c|lib/int/divdi3.c|lib/int/moddi3.c|lib/int/udivmod_w.c) [ $c = 68000 ] || continue;; lib/int/muldi3.c) continue;; esac # 68020: the divu.l members below
    case $src in lib/fp/f64/add_sub.c|lib/fp/f64/mul.c|lib/fp/f64/div.c) continue;; esac # config/m68k/sf64.s replaces them
    LLRM_CC_TARGET=m68k-$c $D/llrm-cc -$opt -S -Iinclude -include config/m68k/config.h ${EXTRA_INC:+-include $EXTRA_INC} $src -o $T/$c/$f.s 2> /dev/null || continue
    awk 'BEGIN{print ".text"} /^(__|sf[0-9]+_)[A-Za-z0-9_]*:$/ {l=$0; sub(/:$/,"",l); print ".globl " l} {print}' $T/$c/$f.s > $T/$c/$f.g.s
    $B-as -m$c -o $T/$c/$f.o $T/$c/$f.g.s 2> /dev/null && ar_objs="$ar_objs $T/$c/$f.o"
  done
  if [ $c = 68000 ]; then LLRM_CC=$D/llrm-cc tools/m68k_div64_members.sh 68000 $T/m$c; ar_objs="$ar_objs $T/m$c/muldi3.o"; fi # the mulu.w asm
  if [ $c = 68020 ]; then LLRM_CC=$D/llrm-cc tools/m68k_div64_members.sh 68020 $T/m$c; ar_objs="$ar_objs $T/m$c/udivdi3.o $T/m$c/umoddi3.o $T/m$c/divdi3.o $T/m$c/moddi3.o $T/m$c/muldi3.o"; fi # what libsf.a ships
  ds=; [ $c = 68020 ] && ds="--defsym M68020=1"
  $B-as -m$c $ds -o $T/$c/mulsi3.o config/m68k/mulsi3.s; ar_objs="$ar_objs $T/$c/mulsi3.o" # __mulsi3 is the asm member
  $B-as -m$c $ds -o $T/$c/sf64.o config/m68k/sf64.s; ar_objs="$ar_objs $T/$c/sf64.o"
  $B-ar rcs $T/$c/ours.a $ar_objs
  gobjs=; for o in $G/${c}_double.o $G/${c}_floatex.o $G/${c}_fpg_$opt.o; do [ -f $o ] && gobjs="$gobjs $o"; done
  LS=${LIBGCC_SRC:-$HOME/scratch/sf-libgcc-src/libgcc}
  for m in eqdf2 nedf2 gtdf2 gedf2 ltdf2 ledf2 cmpdf2; do $B-gcc -msoft-float -m$c -DL_$m -I$LS/config/m68k -I$LS -c $LS/config/m68k/lb1sf68.S -o $T/$c/g_$m.o 2> /dev/null && gobjs="$gobjs $T/$c/g_$m.o"; done
  gobjs="$gobjs $LG2/${c}_*.o"; if [ $c = 68000 ]; then for i in divsi3 modsi3 udivsi3 umodsi3 mulsi3; do gobjs="$gobjs $G/${c}_$i.o"; done; fi # the 68000 has no 32-bit mul/div: libgcc2 calls lb1sf68's
  $B-ar rcs $T/$c/gcc.a $gobjs
  text() { $B-size -A $1 | awk '$1==".text"||$1==".rodata"||$1==".data"{s+=$2} END{print s+0}'; } # code + rodata + data: libgcc's tables are data; text-only by design: a linked executable, ld folds .text.* into .text
  link() { # prog.c archive
    $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -c $1 -o $T/$c/p.o && $B-ld -static -e main -o $T/$c/x $T/$c/p.o $2 2> $T/$c/err && text $T/$c/x || { echo FAIL; [ -n "$VERBOSE" ] && grep -m2 undefined $T/$c/err >&2; }; }
  for col in ours gcc; do link $T/null.c $T/$c/$col.a > $T/$c/null_$col; done
  printf 'extern void NOSUCH(void); void *volatile f = NOSUCH; int main(void) { return f != 0; }\n' > $T/ctl.c
  [ "$(link $T/ctl.c $T/$c/ours.a)" = FAIL ] || { echo "FAIL: control: an undefined symbol linked"; exit 1; }
  for e in $entries; do
    printf 'extern void %s(void); void *volatile f = %s; int main(void) { return f != 0; }\n' $e $e > $T/p.c
    o=$(link $T/p.c $T/$c/ours.a); g=$(link $T/p.c $T/$c/gcc.a)
    n_o=$(cat $T/$c/null_ours); n_g=$(cat $T/$c/null_gcc)
    echo "$c -$opt $e: ours $([ "$o" = FAIL ] && echo - || echo $((o - n_o))) libgcc $([ "$g" = FAIL ] && echo - || echo $((g - n_g))) B"
  done
done
