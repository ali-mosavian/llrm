#!/bin/bash
# usage: LLRM_CC=<llrm-cc> [M68K_ROOT=..] [CPU=68000] [NOSF=1] tests/m68k/rt_smoke.sh -> tools/build_rt_m68k.sh's libsf.a linked into a program that uses double, float and long long on qemu-m68k; the fold must equal the host's.
# Links libsf.a first and without libgcc's float routines (the system libgcc is the hard-float multilib), so a missing member is an undefined symbol, not a wrong answer.
set -e
cd "$(dirname "$0")/../.."; R=$PWD; T=$(mktemp -d); trap 'rm -r $T' EXIT
CPU=${CPU:-68000}; ROOT=${M68K_ROOT:-$HOME/scratch/gcc-m68k/root}; export LD_LIBRARY_PATH=$ROOT/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}; G=$ROOT/usr/bin/m68k-linux-gnu-gcc
bash tools/build_rt_m68k.sh $T/rt $CPU Os > /dev/null
gcc -O0 -DHOST -o $T/host tests/m68k/rt_smoke.c; set +e; $T/host > /dev/null; want=$?; set -e
LLRM_CC_TARGET=m68k-$CPU $LLRM_CC -O2 -S -w tests/m68k/rt_smoke.c -o $T/p.s
$ROOT/usr/bin/m68k-linux-gnu-as -m$CPU -o $T/p.o $T/p.s
$G -m$CPU -O1 -ffreestanding -fno-builtin -nostdlib -c -o $T/start.o tests/m68k/rt_start.c
$G -m$CPU -nostdlib -static -Wl,-Map=$T/map -o $T/p $T/start.o $T/p.o -L$T/rt -L${LIBGCC_DIR:-/usr/lib/gcc-cross/m68k-linux-gnu/15} ${NOSF:+} $([ -z "$NOSF" ] && echo -lsf) -lgcc > $T/link.log 2>&1 || { echo "FAIL: link: $(grep -i -m3 "undefined\|cannot" $T/link.log)"; exit 1; }
if [ -z "$NOSF" ]; then # the program's 64-bit divide is libsf.a's member, and at 68020-040 the divu.l one
  grep -A1 -E '^ \.text +0x[0-9a-f]+ +0x[0-9a-f]+ +.*libsf\.a\(divdi3\.o\)' $T/map > /dev/null || { echo "FAIL: __divdi3 is not libsf.a's"; exit 1; }
  if [ "$CPU" != 68000 ] && [ "$CPU" != 68060 ]; then $ROOT/usr/bin/m68k-linux-gnu-objdump -d $T/p | grep -qE '	divul +%d[0-7],%d[0-7],%d[0-7]$' || { echo "FAIL: no 64/32 divu.l in the linked program"; exit 1; }; fi
fi
set +e; qemu-m68k -cpu m$CPU $T/p; got=$?; set -e
echo "m68k $CPU smoke: exit $got, host $want"
[ "$got" = "$want" ] || { echo "FAIL: differs from the host"; exit 1; }
