#!/bin/sh
# usage: LLRM_CC_DIR=<dir with llrm-cc> tests/m68k_float_split_test.sh
# Before: m68k linked float.c whole for __floatsidf (all four int-to-float entries plus pack): 1076 B at -Os against libgcc's 920-986. With SF_CONV_SPLIT the program links no other
# int-to-float entry, and its linked size (.text) is under 450 B: with SF_NO_EXACT32 (the 64-bit path through pack, "+12..20 B" by the file size, 2.2x the clocks at the link) it was 672 B at 68000 -Os and 1168 clk/call against libgcc's 491. Control: without the knob the other entries are linked (the test measures something).
cd "$(dirname "$0")/.." || exit 2; D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; bad=0
export LD_LIBRARY_PATH=$HOME/scratch/gcc-m68k/root/usr/lib/x86_64-linux-gnu; B=$HOME/scratch/gcc-m68k/root/usr/bin/m68k-linux-gnu
[ -x $B-ld ] || { echo "FAIL: $B-ld not found"; exit 1; }
printf '#undef SF_CONV_SPLIT\n' > $T/nosplit.h
printf 'extern void __floatsidf(void); void *volatile f = __floatsidf; int main(void) { return f != 0; }\n' > $T/p.c
c=68020
for v in split nosplit; do
  inc=; [ $v = nosplit ] && inc="-include $T/nosplit.h"; rm -f "${T:?}"/*.o "${T:?}"/l.a
  for s in float float_si float_usi float_di float_udi pack; do
    LLRM_CC_TARGET=m68k-$c $D/llrm-cc -Os -c -Iinclude -include config/m68k/config.h $inc lib/fp/f64/$s.c -o $T/$s.o || exit 2
  done
  $B-ar rcs $T/l.a $T/float.o $T/float_si.o $T/float_usi.o $T/float_di.o $T/float_udi.o $T/pack.o
  $B-gcc -msoft-float -m$c -O1 -nostdlib -static -ffreestanding -c $T/p.c -o $T/p.o && $B-ld -static -e main -o $T/x $T/p.o $T/l.a 2> $T/err || { echo "FAIL: link $v: $(cat $T/err)"; bad=1; continue; }
  sz=$($B-size -A $T/x | awk '$1==".text"{print $2}'); [ $v = split ] && [ "$sz" -gt 450 ] && { echo "FAIL: a program using only __floatsidf has .text $sz B (bound 450)"; bad=1; }
  n=$($B-nm $T/x | grep -c -E ' __float(un)?didf$| __floatunsidf$')
  [ $v = split ] && [ "$n" -ne 0 ] && { echo "FAIL: a program using only __floatsidf links $n other int-to-float entries"; bad=1; }
  [ $v = nosplit ] && [ "$n" -eq 0 ] && { echo "FAIL: control: unsplit float.c no longer links the other entries"; bad=1; }
done
[ $bad = 0 ] && echo "m68k_float_split ok"; exit $bad
