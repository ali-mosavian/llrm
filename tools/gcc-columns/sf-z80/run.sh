#!/bin/bash
# usage: run.sh DIR VAR OP SYMBOL [N]: fails and cycles/call on ucsim_z80 for one gcc op (VAR = sf32 | sf32_6502).
# DIR holds the .rel files of build.sh. VEC=file replaces the generated records; OBJS="a.rel b.rel" replaces the linked objects.
# Code and records must end below the data at 0xc000: a run whose layout overlaps is refused, not measured.
here=$(cd "$(dirname "$0")" && pwd); G=$here/../../../runtime/compiler-rt/tests/z80; H=$here/../rt-z80
d=$(cd "$1" && pwd); V=$2; op=$3; sym=$4; N=${5:-700}
T=$(mktemp -d); cd "$T"; ulimit -v 16000000
case $op in itof|utof|ftoi|ftou) NA=1;; *) NA=2;; esac
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > crt0.s; sdasz80 -o crt0.rel crt0.s
if [ -n "$VEC" ]; then cp "$VEC" vec.inc; else python3 "$G/gen_f32.py" "$op" > vec.inc || exit 1; fi
# NaN-input records expect the hand asm's canonical NaN; sf32.c propagates the payload: drop them
case $op in itof|utof) dropped=0;; *) dropped=$(python3 -I "$here/filter_nan.py" $NA);; esac
nv=$(sed -n 's/^NV = //p' vec.inc)
sed "s/\\bOP\\b/$sym/g; s/NARGS-1/$NA-1/g" "$here/drvg.s" > drvg.s
for f in "$H"/*.s; do cp "$f" .; sdasz80 -o "$(basename "$f" .s).rel" "$(basename "$f")"; done
ls *.rel | grep -v -e "^crt0" -e "^drvg" -e "^stub" > rt.lib
if [ -n "$OBJS" ]; then OBJ=$OBJS; else [ "$V" = sf32 ] && OBJ="$d/f32_*.rel" || OBJ="$d/x6502/*.rel"; OBJ="$OBJ $d/int32.rel"; fi
# stub run: the same records, a bare ret
printf '\t.module stub\n\t.area _CODE\n\t.globl %s\n%s:\n\tret\n' "$sym" "$sym" > stub.s; sdasz80 -o stub.rel stub.s
sdasz80 -o drvg.rel drvg.s || exit 1
LK="-b _HEADER=0 -b _CODE=0x100 -b _DATA=0xc000"
sdldz80 -i -m $LK s.ihx crt0.rel drvg.rel stub.rel >/dev/null || exit 1
sdldz80 -i -m $LK r.ihx -k "$T" -l rt crt0.rel drvg.rel $OBJ >link.out 2>&1 || { echo "link failed: $(head -3 link.out)"; exit 1; }
code() { awk '$1=="_CODE" && $2 ~ /^[0-9A-F]+$/ {print $'$1'; exit}' "$2"; }
end=$(( 16#$(code 2 r.map) + 16#$(code 3 r.map) ))
[ $end -lt $((0xc000)) ] || { echo "code and records overlap the data: they end at $(printf 0x%x $end), the data starts at 0xc000"; exit 1; }
run() { printf 'run\nstate\ndump rom 0xcf00 0xcf01\ndump rom 0xcf10 0xcf1f\nquit\n' | timeout 600 ucsim_z80 -t Z80 -q -w "$T/$1.ihx" -c - 2>&1; }
os=$(run s); b=$(echo "$os" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
o=$(run r);  c=$(echo "$o"  | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
fb=$(echo "$os" | sed -n 's/^0xcf00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
f2=$(echo "$o"  | sed -n 's/^0xcf00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
bytes=$(( $(code 3 r.map | { read h; echo $((16#$h)); }) - $(code 3 s.map | { read h; echo $((16#$h)); }) + 1 ))
[ "${f2:-ffff}" != 0000 ] && echo "$o" | grep -E "^0xcf1" | head -3
echo "$d $V $op stubfails=$((0x${fb:-ffff})) fails=$((0x${f2:-ffff})) cycles/call=$(( (c-b)/nv )) lib_bytes=$bytes nv=$nv nan_dropped=$dropped"
cp r.map "$d/map.$V.$op.txt" 2>/dev/null; rm -rf "$T"
