#!/bin/sh
# usage: [OLDSHIM=1] bench.sh -> cycles per call and code bytes on the Woz comparison operands (64 records), call minus a bare rts
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); I=../../src/6502
python3 ../woz_compare/gen.py $T 64
[ -n "$OLDSHIM" ] && S= || S=-DSTACKB=1
ca65 -t sim6502 -I $I $S $I/sf32.s -o $T/sf.o; ca65 -t sim6502 -I $I $S $I/sf32_muldiv.s -o $T/md.o; ca65 -t sim6502 -I $I $I/sf32_abi.s -o $T/abi.o; [ -n "$OLDSHIM" ] || : > $T/abi.s; [ -n "$OLDSHIM" ] || ca65 -t sim6502 $T/abi.s -o $T/abi.o
printf '.export __addsf3,__subsf3,__mulsf3,__divsf3\n__addsf3:\n__subsf3:\n__mulsf3:\n__divsf3:\n rts\n' > $T/stub.s; ca65 -t sim6502 $T/stub.s -o $T/stub.o
for op in add sub mul div; do
  python3 - $T/ours_$op.h > $T/vec.inc <<'PY'
import re,sys
t=open(sys.argv[1]).read()
v={k:[int(x,16) for x in re.findall(r'0x([0-9A-F]+)u',re.search(k+r'\[\d+\] = \{(.*?)\}',t).group(1))] for k in 'ABW'}
print("nrec = %d\nvec:"%len(v['A']))
for a,b,w in zip(v['A'],v['B'],v['W']): print("  .dword $%08x, $%08x, $%08x"%(a,b,w))
PY
  printf '.import __%ssf3
OP = __%ssf3
' $op $op > $T/op.inc
  ca65 -t sim6502 -U -I $T drv.s -o $T/drv.o
  ld65 -t sim6502 $T/drv.o $T/stub.o sim6502.lib -o $T/t.bin; set +e; b=$(sim65 -c $T/t.bin 2>&1 | cut -d' ' -f1); set -e
  ld65 -t sim6502 $T/drv.o $T/sf.o $T/md.o $T/abi.o sim6502.lib -o $T/t.bin; set +e; c=$(sim65 -c $T/t.bin 2>&1 | cut -d' ' -f1); echo "$op fails=$? cycles/call=$(( (c-b)/64 ))"; set -e
done
od65 --dump-segsize $T/sf.o | grep CODE; od65 --dump-segsize $T/md.o | grep CODE
rm -r $T
