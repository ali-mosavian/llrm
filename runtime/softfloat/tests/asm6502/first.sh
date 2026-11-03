#!/bin/sh
# usage: first.sh op n -> prints the first failing record (a, b, expected, got)
cd "$(dirname "$0")"; T=$(mktemp -d); I=../../src/6502; op=$1; N=$2
ca65 -t sim6502 -I $I $I/sf32.s -o $T/sf.o; ca65 -t sim6502 -I $I $I/sf32_muldiv.s -o $T/md.o
python3 gen.py $op $N > $T/vec.inc
case $op in sub) D=-DSUBOP=1;; mul) D=-DMULOP=1;; div) D=-DDIVOP=1;; *) D=;; esac
v=""
for s in 0 1 2 3 4 5; do
  ca65 -t sim6502 -U -I $T $D -DSEL=$s drv.s -o $T/drv.o; ld65 -t sim6502 $T/drv.o $T/sf.o $T/md.o sim6502.lib -Ln $T/l -o $T/t.bin
  sim65 -c $T/t.bin >/dev/null; v="$v $(printf %02x $?)"
done
python3 - $T/l "$v" $T/vec.inc <<'PY'
import sys
base=int([l for l in open(sys.argv[1]) if l.strip().endswith('.vec')][0].split()[1],16)
v=[int(x,16) for x in sys.argv[2].split()]
i=((v[1]<<8|v[0])-base)//12
r=[l for l in open(sys.argv[3]) if l.startswith('  .dword')][i]
print(i,r.strip(),'got',''.join('%02x'%b for b in v[2:][::-1]))
PY
rm -r $T
