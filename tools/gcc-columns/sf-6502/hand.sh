#!/bin/bash
# usage: hand.sh: fails and cycles/call of config/6502's hand asm over the same records and the same base-run method as run.sh.
here=$(cd "$(dirname "$0")" && pwd); G=$here/../../../runtime/compiler-rt/tests/asm6502; I=$here/../../../runtime/compiler-rt/config/6502
T=$(mktemp -d); cd "$T"; ulimit -v 16000000
for f in sf32 sf32_muldiv sf32_conv; do ca65 -t sim6502 -I "$I" -DSTACKB=1 "$I/$f.s" -o $f.o || exit 1; done
for spec in add:__addsf3 sub:__subsf3 mul:__mulsf3 div:__divsf3 itof:__floatsisf utof:__floatunsisf ftoi:__fixsfsi ftou:__fixunssfsi; do
  op=${spec%%:*}; sym=${spec#*:}; D=; case $op in itof|utof|ftoi|ftou) D=-DEXACT=1;; esac
  python3 "$G/gen.py" $op 256 > vec.inc; nv=$(sed -n 's/^nrec = //p' vec.inc); printf '.import %s\nOP = %s\n' $sym $sym > op.inc
  ca65 -t sim6502 -U -I . $D "$G/drv.s" -o drv.o
  mkdir -p b; python3 -c "print('nrec = $nv\nvec:'); print('  .dword 0, 0, 0\n' * $nv)" > b/vec.inc; cp op.inc b/
  ca65 -t sim6502 -U -I b $D "$G/drv.s" -o b/drv.o
  printf '.export %s\n%s: lda #0\n ldx #0\n sta $82\n sta $83\n rts\n' $sym $sym > stub.s; ca65 -t sim6502 stub.s -o stub.o
  ld65 -C "$G/sim6502_lut.cfg" -o s.bin b/drv.o stub.o sim6502.lib; sim65 s.bin >/dev/null || { echo "$op: base run must pass"; exit 1; }
  ld65 -C "$G/sim6502_lut.cfg" -o r.bin drv.o sf32.o sf32_muldiv.o sf32_conv.o sim6502.lib 2>link.out || { echo "$op: $(head -2 link.out)"; exit 1; }
  b=$(sim65 -c s.bin 2>&1 | sed -n 's/^\([0-9]*\) cycles.*/\1/p'); c=$(sim65 -c r.bin 2>&1 | sed -n 's/^\([0-9]*\) cycles.*/\1/p'); sim65 r.bin >/dev/null; f=$?
  echo "hand $op fails=$f cycles/call=$(( (c-b)/nv )) nv=$nv"
done
for f in sf32 sf32_muldiv sf32_conv; do echo "hand bytes $f.s $(od65 --dump-segsize $f.o | awk '/(CODE|RODATA|DATA):/{s+=$2} END{print s}')"; done
rm -rf "$T"
