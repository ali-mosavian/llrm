#!/bin/sh
# usage: tests/m68k_asm_clocks.sh OUTDIR [vectors.h]: asm clocks per call at 68000/68020/68060 via `cycles --exe`.
# `cycles` picks the qemu CPU from the exe name (m68000 unless "68020"): a 68060 exe ran on a 68000, died on its first
# 68020+ instruction and priced 339 instructions as 0.1 clk/call. CYCLES_QEMU_CPU is set and a short trace fails the run.
set -e
cd "$(dirname "$0")/.."
O=$1; : "${CYCLES:?CYCLES=path to the llrm-m68k cycles example}"; export GCC_M68K=${GCC_M68K:-$HOME/scratch/gcc-m68k/root}
for c in 68000 68020 68060; do
  tests/m68k_asm.sh clocks $c "$O" $2
  CYCLES_QEMU_CPU=m$c CYCLES_CPUS=$c $CYCLES --exe "$O"/asm_${c}_op[0-4] | tee "$O/clocks_$c.md" | grep '^| asm'
  awk -F'|' '/^\| asm/ && $3+0 < 10000 { print "short trace: " $2; bad=1 } END { exit bad }' "$O/clocks_$c.md"
done
