#!/bin/sh
# usage: [OLDSHIM=1] bench.sh -> cycles per call and code bytes on the Woz comparison operands (64 records), call minus a stub that returns 0 (base run passes its records)
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); I=../../config/6502
python3 ../woz_compare/gen.py $T 64
[ -n "$OLDSHIM" ] && S= || S=-DSTACKB=1
ca65 -t sim6502 -I $I $S $I/sf32.s -o $T/sf.o; ca65 -t sim6502 -I $I $S $I/sf32_muldiv.s -o $T/md.o; ca65 -t sim6502 -I $I $I/sf32_abi.s -o $T/abi.o; [ -n "$OLDSHIM" ] || : > $T/abi.s; [ -n "$OLDSHIM" ] || ca65 -t sim6502 $T/abi.s -o $T/abi.o
# base: the same driver and library, 64 zero records through a stub that returns 0, so every record passes
printf '.export stub_op\nstub_op:\n lda #0\n ldx #0\n sta $82\n sta $83\n rts\n' > $T/stub.s; ca65 -t sim6502 $T/stub.s -o $T/stub.o
mkdir $T/b; python3 -c "print('nrec = 64\nvec:'); print('  .dword 0, 0, 0\n' * 64)" > $T/b/vec.inc; printf '.import stub_op\nOP = stub_op\n' > $T/b/op.inc
ca65 -t sim6502 -U -I $T/b drv.s -o $T/b/drv.o
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
  ld65 -t sim6502 $T/b/drv.o $T/stub.o $T/sf.o $T/md.o $T/abi.o sim6502.lib -o $T/t.bin; set +e; sim65 -c $T/t.bin > $T/o 2>&1; r=$?; set -e; b=$(cut -d' ' -f1 $T/o)
  [ $r = 0 ] || { echo "$op: the base run must pass its records (a failing one pays the driver's report loop), got $r"; exit 1; }
  ld65 -t sim6502 $T/drv.o $T/sf.o $T/md.o $T/abi.o sim6502.lib -o $T/t.bin; set +e; sim65 -c $T/t.bin > $T/o 2>&1; r=$?; set -e; c=$(cut -d' ' -f1 $T/o); echo "$op fails=$r cycles/call=$(( (c-b)/64 ))"
done
od65 --dump-segsize $T/sf.o | grep CODE; od65 --dump-segsize $T/md.o | grep CODE
rm -r $T
