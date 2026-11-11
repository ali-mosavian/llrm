#!/bin/bash
# usage: LLRM_CC_DIR=dir [GCC_M68K=root] tests/m68k_bitcount_bfffo.sh
# The 68020 and later count with bfffo (config/m68k/config.h, SF_BFFFO): __clzsi2 __ctzsi2 __clzdi2 __ctzdi2 hold the instruction and call nobody; the 68000 and 68010 keep the
# ladder. Then the four routines run on qemu at 68020, 68030, 68040 and 68060 against loops (tests/m68k_bitcount_main.c). Fails if the 68020 keeps the 76 B ladder.
set -e
cd "$(dirname "$0")/.."; D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; T=$(mktemp -d); trap 'rm -r $T' EXIT
FLAGS="-Iinclude -include config/m68k/config.h"
S() { LLRM_CC_TARGET=m68k-$1 $D/llrm-cc -Os -S $FLAGS lib/int/$2.c -o $T/$2.s; }
for cpu in 68020 68030 68040 68060; do
  for r in clzsi2 ctzsi2 clzdi2 ctzdi2; do
    S $cpu $r
    grep -q 'bfffo' $T/$r.s || { echo "FAIL: $cpu __$r has no bfffo"; exit 1; }
    ! grep -q 'jsr' $T/$r.s || { echo "FAIL: $cpu __$r calls a routine"; exit 1; }
  done
done
for r in clzsi2 ctzsi2; do S 68000 $r; ! grep -q 'bfffo' $T/$r.s || { echo "FAIL: 68000 __$r uses bfffo"; exit 1; }; done
grep -q 'jsr' $T/ctzsi2.s || { echo "FAIL: 68000 __ctzsi2 does not call __clzsi2"; exit 1; }
for cpu in 68020 68030 68040 68060; do
  QEMU_CPU=m$cpu CFLAGS_EXTRA="$FLAGS" LLRM_CC_DIR=$D tests/m68k_run_c.sh $cpu Os tests/m68k_bitcount_main.c lib/int/clzsi2.c lib/int/ctzsi2.c lib/int/clzdi2.c lib/int/ctzdi2.c > $T/run.txt 2>&1 || { echo "FAIL: $cpu: a count is wrong (the program exits with their number) or it did not link"; cat $T/run.txt; exit 1; }
  grep -qx 'exit=0' $T/run.txt || { echo "FAIL: $cpu: $(tail -1 $T/run.txt)"; exit 1; }
done
echo "ok: bfffo routines at 68020/030/040/060, ladder at 68000"
