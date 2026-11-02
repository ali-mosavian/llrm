#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [FP8_M=2] [OPT=Os] tests/asm6502/bench8c.sh
# src/sf8.c compiled by llrm-cc for 6502-sim65: mean cycles per call over chunk 3 (4096 pairs, call minus a bare rts)
# and the mismatches in that chunk. Correctness of the C itself is tests/host_fp8.c.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); S=../..; M=${FP8_M:-2}; OPT=${OPT:-Os}
MOS_RT=${MOS_RT:-$HOME/scratch/handoff/mos_rt.s}
$LLRM_CC --target=6502-sim65 -$OPT -S -DFP8_M=$M -I$S/include '-DSF8_NAME(n)=__fp8_##n' $S/src/sf8.c -o $T/c.s
grep -h '^rc[0-9]* = ' $T/c.s | sort -u > $T/eq.s
sed 's/^\.include "op.inc"//' drv8.s > $T/drv.s
for op in add sub mul div; do
  python3 gen8.py $op 3 > $T/exp8.inc
  { cat $T/eq.s; echo ".include \"exp8.inc\""; printf 'OP = __fp8_%s\n' $op; cat $T/drv.s | grep -v '^\.include "exp8.inc"'; grep -v '^rc[0-9]* = ' $T/c.s; cat $MOS_RT; } > $T/p.s
  { cat $T/eq.s; echo ".include \"exp8.inc\""; printf 'OP = __fp8_%s\n' $op; cat $T/drv.s | grep -v '^\.include "exp8.inc"'; printf '__fp8_%s:\n rts\n' $op; } > $T/b.s
  cl65 -t sim6502 -I $T -DFP8_M=$M -o $T/p.bin $T/p.s; cl65 -t sim6502 -I $T -DFP8_M=$M -o $T/b.bin $T/b.s
  set +e; c=$(sim65 -c $T/p.bin 2>&1 | cut -d' ' -f1); f=$(sim65 -c $T/p.bin >/dev/null 2>&1; echo $?); b=$(sim65 -c $T/b.bin 2>&1 | cut -d' ' -f1); set -e
  echo "$op cycles/call=$(( (c-b)/4096 )) mismatches=$f"
done
rm -r $T
