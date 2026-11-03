#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc built from a tree that has the uint64 carry fix> [CPUS="68000 68020"] tests/m68k_f64_rerun.sh
# The f64 rerun in one command: int64 harness, f64 mismatches per op and class (60000 vectors), then the pricing executables.
# Prints the compiler's sha first so a number is never quoted for the wrong build. Per-function split of sf64.c: tests/m68k_f64_split.sh.
set -e
cd "$(dirname "$0")/.."
D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; RUN=${M68K_RUN_C:-$HOME/scratch/handoff/m68k-run-c.sh}; T=$(mktemp -d); trap 'rm -r $T' EXIT
strings $D/llrm-llc | grep -m1 -E '^[0-9a-f]{40}$' || echo "compiler sha: not stamped in $D"
export LLRM_CC_DIR=$D
for c in ${CPUS:-68000 68020}; do for opt in Os O2; do
  echo "== int64 $c $opt"; (cd tests/m68k_int64 && python3 gen.py > $T/v.h && CFLAGS_EXTRA="-I$T -I. -I../../include" sh $RUN $c $opt main.c ops64.c ../../lib/int/int64.c ../../lib/int/int32.c)
  echo "== f64 classes $c $opt"; cc -O2 -ffp-contract=off tests/gen_vectors64.c -o $T/gen64 && $T/gen64 60000 > $T/vectors64.h
  CFLAGS_EXTRA="-I$T -Iinclude" sh $RUN $c $opt tests/m68k_classes64.c lib/fp/sf64.c lib/fp/sf32_conv64.c lib/int/int64.c lib/int/int32.c
done; done
echo "== pricing executables"; tests/build_m68k_clocks64.sh
echo "price with: cycles --exe ~/scratch/handoff/m68k-clocks/{ours,libgcc}64_*"
