#!/bin/sh
# usage: [FP8_ROWS=16] bench8.sh -> bytes of sf8.s and mean cycles per call over ROWS*256 pairs (chunk 3)
# Per call = (run - base) / records + 7, where base calls a stub that returns the expected byte (ldy #0, lda ($a0),y, rts = 7 cycles,
# so the base takes the driver's passing path like the real run). A run with mismatches is refused: its failing branch costs more.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); I=../../config/6502; ca65 -t sim6502 -I $I -DFP8_M=${FP8_M:-2} -DFP8_MULTAB=${FP8_MULTAB:-0} ${SF8_S:-$I/sf8.s} -o $T/sf8.o; od65 --dump-segsize $T/sf8.o | grep -E "CODE|RODATA"
printf '.export __fp8_add,__fp8_sub,__fp8_mul,__fp8_div\n__fp8_add:\n__fp8_sub:\n__fp8_mul:\n__fp8_div:\n ldy #0\n lda ($a0),y\n rts\n' > $T/s.s; ca65 -t sim6502 $T/s.s -o $T/s.o
for op in add sub mul div; do printf '.import __fp8_%s\nOP = __fp8_%s\n' $op $op > $T/op.inc; python3 gen8.py $op 3 > $T/exp8.inc; ca65 -t sim6502 -U -I $T drv8.s -o $T/d.o
 n=$(( $(sed -n 's/^ROWS = //p' $T/exp8.inc) * 256 ))
 ld65 -t sim6502 $T/d.o $T/s.o sim6502.lib -o $T/t.bin; set +e; b=$(sim65 -c $T/t.bin 2>$T/err | cut -d' ' -f1); bf=$?; bx=$(sim65 -c $T/t.bin >/dev/null 2>&1; echo $?)
 ld65 -t sim6502 $T/d.o $T/sf8.o sim6502.lib -o $T/t.bin; c=$(sim65 -c $T/t.bin 2>&1|cut -d' ' -f1); cx=$(sim65 -c $T/t.bin >/dev/null 2>&1; echo $?); set -e
 [ "$bx" = 0 ] || { echo "$op: base run fails $bx records, refusing" >&2; exit 1; }
 [ "$cx" = 0 ] || { echo "$op: $cx mismatches, refusing" >&2; exit 1; }
 echo $op $(( (c-b)/n + 7 )); done; rm -r $T
