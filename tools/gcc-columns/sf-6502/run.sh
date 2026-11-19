#!/bin/bash
# usage: run.sh DIR VAR OP SYMBOL [N]: fails, cycles/call and library bytes on sim65 for one gcc op (VAR = generic | x6502; DIR from build.sh).
# clk = (run - bare-rts stub run)/records; bytes = CODE+RODATA+DATA of every member the link takes from the library (hand helpers included).
here=$(cd "$(dirname "$0")" && pwd); G=$here/../../../runtime/compiler-rt/tests/asm6502
d=$(cd "$1" && pwd); V=$2; op=$3; sym=$4; N=${5:-256}
T=$(mktemp -d); cd "$T"; ulimit -v 16000000
python3 "$G/gen.py" "$op" "$N" > vec.inc || exit 1
dropped=0; [ "$V" = x6502 ] && case $op in ftoi|ftou) dropped=$(python3 -I "$here/filter_nan.py");; esac
nv=$(sed -n 's/^nrec = //p' vec.inc)
printf '.import %s\nOP = %s\n' "$sym" "$sym" > op.inc
D=; case $op in add|sub|mul|div) D="-DBIN=1";; esac; case $op in itof|utof|ftoi|ftou) D="-DEXACT=1";; esac
ca65 -t sim6502 -U -I . $D "$here/drv.s" -o drv.o || exit 1
# base run: nv zero records through a stub that returns 0, so every record passes (a failing record pays the driver's report path)
mkdir b; python3 -c "print('nrec = $nv\nvec:'); print('  .dword 0, 0, 0\n' * $nv)" > b/vec.inc; cp op.inc b/
ca65 -t sim6502 -U -I b $D "$here/drv.s" -o b/drv.o || exit 1
printf '.importzp rc2, rc3, rc4, rc5\n.export %s\n.segment "CODE"\n%s: lda #0\n sta rc2\n sta rc3\n sta rc4\n sta rc5\n rts\n' "$sym" "$sym" > stub.s; ca65 -t sim6502 -o stub.o stub.s
CFG=$G/sim6502_lut.cfg
ld65 -C "$CFG" -o s.bin b/drv.o stub.o sim6502.lib || exit 1
sim65 -x 500000000 s.bin >/dev/null 2>&1 || { echo "base run must pass its records"; exit 1; }
ld65 -C "$CFG" -m r.map -o r.bin drv.o "$d/$V.lib" sim6502.lib >link.out 2>&1 || { echo "link failed: $(head -3 link.out)"; exit 1; }
cycles() { sim65 -c -x 500000000 "$1" 2>&1 | sed -n 's/^\([0-9]*\) cycles.*/\1/p'; }
b=$(cycles s.bin); c=$(cycles r.bin)
sim65 -x 500000000 r.bin >/dev/null 2>&1; fails=$?
bytes=$(python3 -I "$here/lib_bytes.py" r.map)
echo "$d $V $op fails=$fails cycles/call=$(( (c-b)/nv )) bytes=$bytes nv=$nv nan_dropped=$dropped"
cp r.map "$d/map.$V.$op.txt" 2>/dev/null; rm -rf "$T"
