#!/bin/bash
# m68k f64 int conversions (float_si/usi/di/udi + pack) linked: SF_NO_EXACT32 made them 1888 B (Os) / 2024 (O2) at 68000; exact32 1760 / 1888.
# usage: LLRM_CC_DIR=dir-with-llrm-cc tests/m68k_conv_size_test.sh
cd "$(dirname "$0")/.." || exit 1
D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; G=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}; B=$G/usr/bin/m68k-linux-gnu; export LD_LIBRARY_PATH=$G/usr/lib/x86_64-linux-gnu
T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT; rc=0
for c in 68000 68020; do for opt in Os O2; do
  objs=; for f in float_si float_usi float_di float_udi pack; do
    LLRM_CC_TARGET=m68k-$c $D/llrm-cc -$opt -S -DSF_INTABI -DSF_CONV_SPLIT -include config/m68k/config.h -Iinclude lib/fp/f64/$f.c -o $T/$f.s || exit 2
    $B-as -m$c -o $T/$f.o $T/$f.s || exit 2; objs="$objs $T/$f.o"; done
  $B-ld -r -o $T/all.o $objs || exit 2; got=$($B-size $T/all.o | awk 'NR==2{print $1}'); cap=1780; [ $opt = O2 ] && cap=1920
  echo "m68k-$c $opt $got B (cap $cap)"; [ "$got" -le "$cap" ] || rc=1
done; done; exit $rc
