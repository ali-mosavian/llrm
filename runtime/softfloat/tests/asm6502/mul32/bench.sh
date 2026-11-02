#!/bin/sh
# usage: bench.sh -> per variant: bytes (code+table), mismatches over 64 vectors, mean cycles per call (call minus bare rts)
set -e
cd "$(dirname "$0")"; T=$(mktemp -d)
python3 - > $T/vec.inc <<'P'
import random
random.seed(7)
v=[(0,0),(1,1),(0xffffffff,0xffffffff),(0xffffffff,1),(0x80000000,2),(0x12345678,0x9abcdef0),(0xff,0xff),(0xffff,0xffff)]
v+=[(random.getrandbits(32),random.getrandbits(32)) for _ in range(56)]
print("NV=%d\nvec:"%len(v))
for a,b in v:
    print(".dword $%08x,$%08x,$%08x"%(a,b,(a*b)&0xffffffff))
P
printf '.export mul32\nmul32: rts\n' > $T/s.s; ca65 -t sim6502 $T/s.s -o $T/s.o
ca65 -t sim6502 -I $T drv.s -o $T/d.o; ld65 -t sim6502 $T/d.o $T/s.o sim6502.lib -o $T/t.bin
set +e; b=$(sim65 -c $T/t.bin 2>&1|cut -d' ' -f1); set -e
for v in 1 2 3 4; do
  ca65 -t sim6502 -DVAR=$v mul32.s -o $T/m.o; ld65 -t sim6502 $T/d.o $T/m.o sim6502.lib -o $T/t.bin
  sz=$(od65 --dump-segsize $T/m.o | awk '/CODE|RODATA/{s+=$2} END{print s}')
  set +e; o=$(sim65 -c $T/t.bin 2>&1); f=$?; set -e
  c=$(echo "$o"|cut -d' ' -f1)
  echo "var$v bytes=$sz fails=$f cycles=$(( (c-b)/64 ))"
done
rm -r $T
