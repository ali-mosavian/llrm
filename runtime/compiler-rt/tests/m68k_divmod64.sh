#!/bin/sh
# 64-bit quotient and remainder in one call (lib/int/udivmoddi4.c, divmoddi4.c over udivmod_w.c) against host vectors under qemu-m68k.
#   tests/m68k_divmod64.sh [CPU...]      default 68000 68060 (the CPUs whose 64-bit divide is the C routine; has_div_rem_op(64) there).
# LLRM_CC_DIR=dir (llrm-cc): the call sites of tests/m68k_divmod64_calls.c are built by it, and the link has no __divdi3/__moddi3,
# so a site that does not reach the pair fails to link. A control built on vectors with one wrong remainder must fail. Env: GCC_M68K (default ~/scratch/gcc-m68k/root).
set -e
ulimit -v 16777216
cd "$(dirname "$0")/.."
R=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$R/usr/bin/m68k-linux-gnu
export LD_LIBRARY_PATH=$R/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r $T' EXIT
[ $# -gt 0 ] || set -- 68000 68060
cc -O2 tests/gen_div64.c -o $T/gen && $T/gen 3000 > $T/vectors.h
for c in "$@"; do
  mkdir -p $T/$c
  for f in udivmoddi4 divmoddi4 udivmod_w; do $B-gcc -m$c -msoft-float -Os -ffreestanding -Iinclude -c lib/int/$f.c -o $T/$c/$f.o; done
  VIA=; if [ -n "$LLRM_CC_DIR" ]; then
    LLRM_CC_TARGET=m68k-$c $LLRM_CC_DIR/llrm-cc -O2 -S -Iinclude tests/m68k_divmod64_calls.c -o $T/calls.s && $B-as -m$c -o $T/$c/calls.o $T/calls.s; VIA=-DVIA_CALLS
  fi
  $B-gcc -m$c -msoft-float -O1 -nostdlib -static -ffreestanding $VIA -I$T -c tests/m68k_divmod64_main.c -o $T/main$c.o
  $B-ld -static -e _start -o $T/$c/exe $T/main$c.o $T/$c/*.o
  qemu-m68k -cpu m$c $T/$c/exe && echo "$c: exit=0${VIA:+ (llrm-cc call sites)}" || { echo "$c: exit=$?"; exit 1; }
  # control: the vectors with one wrong remainder must fail
  mkdir -p $T/bad; awk '/^{/ && !done++ && NR > 3 {n=split($0,a,","); sub(/0x[0-9a-f]*u/, "0xdeadbeefu", a[7]); s=a[1]; for (i=2;i<=n;i++) s=s","a[i]; print s; next} {print}' $T/vectors.h > $T/bad/vectors.h
  $B-gcc -m$c -msoft-float -O1 -nostdlib -static -ffreestanding $VIA -I$T/bad -c tests/m68k_divmod64_main.c -o $T/bad/main.o && $B-ld -static -e _start -o $T/bad/exe $T/bad/main.o $T/$c/*.o
  if qemu-m68k -cpu m$c $T/bad/exe; then echo "$c: control: a wrong expected remainder passed"; exit 1; fi
done
