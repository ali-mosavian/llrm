#!/bin/sh
# usage: bench8.sh -> bytes of sf8.s and mean cycles per call over 4096 pairs (chunk 3), call minus a bare rts
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); I=../../src/6502; ca65 -t sim6502 -I $I -DFP8_M=${FP8_M:-2} $I/sf8.s -o $T/sf8.o; od65 --dump-segsize $T/sf8.o | grep CODE
printf '.export __fp8_add,__fp8_sub,__fp8_mul,__fp8_div\n__fp8_add:\n__fp8_sub:\n__fp8_mul:\n__fp8_div:\n rts\n' > $T/s.s; ca65 -t sim6502 $T/s.s -o $T/s.o
for op in add sub mul div; do printf '.import __fp8_%s\nOP = __fp8_%s\n' $op $op > $T/op.inc; python3 gen8.py $op 3 > $T/exp8.inc; ca65 -t sim6502 -U -I $T drv8.s -o $T/d.o
 ld65 -t sim6502 $T/d.o $T/s.o sim6502.lib -o $T/t.bin; set +e; b=$(sim65 -c $T/t.bin 2>&1|cut -d' ' -f1)
 ld65 -t sim6502 $T/d.o $T/sf8.o sim6502.lib -o $T/t.bin; c=$(sim65 -c $T/t.bin 2>&1|cut -d' ' -f1); set -e; echo $op $(( (c-b)/4096 )); done; rm -r $T
