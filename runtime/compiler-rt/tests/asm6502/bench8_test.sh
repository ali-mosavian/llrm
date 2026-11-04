#!/bin/sh
# bench8.sh must report the cycles of a routine whose cost is known: 7 + 2K for `ldy #0; lda ($a0),y; K nops; rts`, with ROWS*256 records.
# Was wrong twice: the base stub (bare rts) returned garbage, so its run paid the driver's failing branch (~17 cycles per record) and every
# figure came out low; the divisor was a hard-coded 4096.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); trap 'rm -r $T' EXIT
for K in 0 10; do
  { printf '.export __fp8_add,__fp8_sub,__fp8_mul,__fp8_div\n__fp8_add:\n__fp8_sub:\n__fp8_mul:\n__fp8_div:\n ldy #0\n lda ($a0),y\n'; i=0; while [ $i -lt $K ]; do echo ' nop'; i=$((i+1)); done; echo ' rts'; } > $T/fake.s
  for R in 16 8; do
    FP8_ROWS=$R SF8_S=$T/fake.s sh bench8.sh 2>&1 | awk -v want=$((7 + 2*K)) -v k=$K -v r=$R '$1 ~ /^(add|sub|mul|div)$/ { n++; if ($2 != want) { print "K=" k " rows=" r ": " $1 " reports " $2 ", want " want; bad=1 } } END { exit bad || n != 4 }'
  done
done
echo bench8 ok
