#!/bin/sh
# 64-bit divide libcalls on the m68k: config/m68k/div64/*.s (divu.l 64/32, 68020/030/040) against host vectors under qemu-m68k.
#   tests/m68k_div64.sh [CPU...]      default 68020 68040. LIB=c links the C lib/int instead of the asm (the check for divu.l then fails).
#   tests/m68k_div64.sh size CPU      one line per entry: bytes of that entry linked alone (entry + core), null program subtracted.
# 68000 and 68060 assemble empty (they keep the C): checked. Env: GCC_M68K (default ~/scratch/gcc-m68k/root).
set -e
ulimit -v 16777216
cd "$(dirname "$0")/.."
R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$R/usr/bin/m68k-linux-gnu
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
ENTRIES="udivdi3 umoddi3 divdi3 moddi3"
objs() { # cpu dir -> asm objects for the entries (the 68000 and 68060 assemble empty)
  DEF=; [ "$1" = 68000 ] || DEF="--defsym M68020=1"; [ "$1" = 68060 ] && DEF="$DEF --defsym M68060=1"
  for f in $ENTRIES; do $B-as -m$1 $DEF -I config/m68k/div64 -o $2/$f.o config/m68k/div64/$f.s; done
}
members() { tools/m68k_div64_members.sh $1 $2; } # what the libsf.a builder takes
cobjs() { # cpu dir -> the C versions
  for f in $ENTRIES udivmod_w; do $B-gcc -m$1 -msoft-float -Os -ffreestanding -Iinclude -c lib/int/$f.c -o $2/c_$f.o; done
}
if [ "$1" = size ]; then # bytes.sh's instrument: link only this entry (+ null program), per column; LIBGCC2_OBJS adds the stock libgcc2 members
  c=$2; mkdir -p $T/$c; objs $c $T/$c; $B-ar rcs $T/$c/ours.a $T/$c/*.o
  if [ -n "$LIBGCC2_OBJS" ]; then $B-ar rcs $T/$c/gcc.a $LIBGCC2_OBJS/${c}_*.o; fi
  text() { $B-size -A $1 | awk '$1==".text"{print $2}'; }
  link() { $B-gcc -m$c -O1 -nostdlib -static -ffreestanding -c $1 -o $T/p.o && $B-ld -static -e main -o $T/x $T/p.o $2 2> /dev/null && text $T/x || echo FAIL; }
  printf 'int main(void) { return 0; }\n' > $T/null.c; n=$(link $T/null.c $T/$c/ours.a)
  printf 'extern void NOSUCH(void); void *volatile f = NOSUCH; int main(void) { return f != 0; }\n' > $T/ctl.c
  [ "$(link $T/ctl.c $T/$c/ours.a)" = FAIL ] || { echo "FAIL: control: an undefined symbol linked"; exit 1; }
  for e in $ENTRIES; do
    printf 'extern void __%s(void); void *volatile f = __%s; int main(void) { return f != 0; }\n' $e $e > $T/p.c
    o=$(link $T/p.c $T/$c/ours.a); line="$c __$e: ours $((o - n)) B"
    if [ -f $T/$c/gcc.a ]; then g=$(link $T/p.c $T/$c/gcc.a); gn=$(link $T/null.c $T/$c/gcc.a); line="$line libgcc $((g - gn)) B"; fi
    echo "$line"
  done
  exit 0
fi
[ $# -gt 0 ] || set -- 68020 68030 68040
for c in 68000 68060; do mkdir -p $T/e$c; objs $c $T/e$c
  for o in $T/e$c/*.o; do [ "$($B-size $o | awk 'NR==2{print $1}')" = 0 ] || { echo "$c: $o is not empty"; exit 1; }; done; done
cc -O2 tests/gen_div64.c -o $T/gen && $T/gen 3000 > $T/vectors.h
for c in "$@"; do
  mkdir -p $T/$c
  if [ "$LIB" = c ]; then cobjs $c $T/$c; else members $c $T/$c; fi
  VIA=; if [ -n "$LLRM_CC_DIR" ]; then
    LLRM_CC_TARGET=m68k-$c $LLRM_CC_DIR/llrm-cc -Os -S -Iinclude tests/m68k_div64_calls.c -o $T/calls.s && $B-as -m$c -o $T/$c/calls.o $T/calls.s; VIA=-DVIA_CALLS
  fi
  $B-gcc -m$c -msoft-float -O1 -nostdlib -static -ffreestanding $VIA -I$T -c tests/m68k_div64_main.c -o $T/main$c.o
  $B-ld -static -e _start -o $T/$c/exe $T/main$c.o $T/$c/*.o
  # objdump prints the 64/32 form `divu.l ea,Dr:Dq` as three operands (divul), the 32-bit one as two
  $B-objdump -d $T/$c/exe | grep -qE '	divul +%d[0-7],%d[0-7],%d[0-7]$' || { echo "$c: no 64/32 divu.l in the linked libcalls"; exit 1; }
  qemu-m68k -cpu m$c $T/$c/exe && echo "$c: exit=0${VIA:+ (llrm-cc call sites)}" || { echo "$c: exit=$?"; exit 1; }
  # control: the same exe built on vectors with one wrong quotient must fail
  mkdir -p $T/bad; awk '/^{/ && !done++ && NR > 3 {n=split($0,a,","); sub(/0x[0-9a-f]*u/, "0xdeadbeefu", a[5]); s=a[1]; for (i=2;i<=n;i++) s=s","a[i]; print s; next} {print}' $T/vectors.h > $T/bad/vectors.h
  $B-gcc -m$c -msoft-float -O1 -nostdlib -static -ffreestanding $VIA -I$T/bad -c tests/m68k_div64_main.c -o $T/bad/main.o && $B-ld -static -e _start -o $T/bad/exe $T/bad/main.o $T/$c/*.o
  if qemu-m68k -cpu m$c $T/bad/exe; then echo "$c: control: a wrong expected quotient passed"; exit 1; fi
done
