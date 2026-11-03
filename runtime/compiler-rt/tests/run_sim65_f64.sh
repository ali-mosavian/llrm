#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [MOS_RT=mos_rt.s] [OPT=Os] [SHOW=n] tests/run_sim65_f64.sh [n] [batch]
# binary64 on the 6502 (sim65) against the host FPU: n standard pairs, then n edge pairs (gen_vectors64_edge.c), batch records per run
# (sim65 has 64 KB: the library takes about 40 KB). The library is compiled once; each batch relinks the driver with its slice.
# Prints, per op and class of the expected result (zerosign, denormal, special, normal): pairs and mismatches; then compares and conversions.
# llrm-cc gaps worked around here, not in the library: the driver is always -O2 (-Os: "greedy refused" on main); static functions are
# exported, so only __*, sf32_pack and sf64_pack stay exported; global data is not exported, so the wrapper is appended to the driver's .s;
# mos_rt.s keeps its __sa/__sb cells private, so they are exported here. int32.c is not linked: the compiler calls __mulsi3 and the
# other 32-bit helpers with operands in mos_rt.s's __sa/__sb, not the C convention.
set -e
cd "$(dirname "$0")/.."
N=${1:-3000}; B=${2:-110}; T=$(mktemp -d); OPT=${OPT:-Os}; SHOW=${SHOW:-0}
MOS_RT=${MOS_RT:?set MOS_RT to the mos_rt.s runtime}
for g in gen_vectors64 gen_vectors64_edge; do cc -O2 -ffp-contract=off -DEXT tests/$g.c -o $T/$g; done
driver() { $LLRM_CC --target=6502-sim65 -O2 -S -DSF_INTABI -DSHOW=$SHOW -Iinclude -I$T tests/sim65_main64.c -o $T/main.s; }
# one object per source file (fp_sources.sh); names are the path with / as _, so f32/mul.c and f64/mul.c stay apart
SRCS="$(tests/fp_sources.sh f32 f32_64 f64) lib/int/int64.c"
LIBS=; for s in $SRCS; do n=$(echo ${s#lib/} | sed 's,[/.],_,g'); LIBS="$LIBS $n"; done
set -- $LIBS; for s in $SRCS; do
  $LLRM_CC --target=6502-sim65 -$OPT -S -DSF_INTABI -Iinclude $s -o $T/$1.s; shift
done
# ld65 links whole objects: keep only what the driver reaches (int64.c would bring its divide for a shift).
$T/gen_vectors64 1 > $T/vectors64.h; driver
mkdir $T/r
python3 tests/sim65_reach.py $T/r main $T/main.s $(for f in $LIBS; do echo $T/$f.s; done) > /dev/null
OBJS=
for f in $LIBS; do
  sed -i '/^\.export \(__\|sf32_pack\|sf64_pack\)/!{/^\.export /d}' $T/r/$f.s
  ca65 -t sim6502 -U $T/r/$f.s -o $T/r/$f.o; OBJS="$OBJS $T/r/$f.o"
done
{ i=0; while [ $i -lt 26 ]; do echo "rc$i = $((128 + i))"; i=$((i + 1)); done; cat $MOS_RT; echo ".export __sa, __sb"; } > $T/mos.s
ca65 -t sim6502 $T/mos.s -o $T/mos_rt.o
# _main: soft stack (rc0:rc1) to the top of a 1 KB buffer, run main, print outbuf, return main's exit code. It is appended to the
# driver's .s: llrm-cc exports no global data.
cat > $T/wrap.s <<'EOS'
.import _write, pushax
.export _main
.bss
sstk: .res 1024
ec: .res 1
.code
_main:
  lda #<(sstk+1024)
  sta rc0
  lda #>(sstk+1024)
  sta rc1
  jsr main
  sta ec
  ldx #0
  lda #1
  jsr pushax
  lda #<outbuf
  ldx #>outbuf
  jsr pushax
  lda outlen
  ldx outlen+1
  jsr _write
  lda ec
  ldx #0
  rts
EOS
: > $T/all.txt
for set in std edge; do
  gen=$T/gen_vectors64; [ $set = edge ] && gen=$T/gen_vectors64_edge
  start=0
  while [ $start -lt $N ]; do
    cnt=$B; [ $((start + cnt)) -gt $N ] && cnt=$((N - start))
    $gen $N $start $cnt > $T/vectors64.h
    driver
    cat $T/main.s $T/wrap.s > $T/full.s
    ca65 -t sim6502 -U $T/full.s -o $T/main.o
    cl65 -t sim6502 -o $T/p.bin $T/main.o $OBJS $T/mos_rt.o
    sim65 -x 4000000000 $T/p.bin > $T/out.txt || true
    [ -s $T/out.txt ] || { echo "no output from batch $set $start"; exit 2; }
    sed "s/^/$set /" $T/out.txt >> $T/all.txt
    start=$((start + cnt))
  done
done
awk -v n_per_set=$N '
  { k = ($2 ~ /^(add|sub|mul|div)$/) ? $1 " " $2 " " $3 : $1 " " $2
    off = ($2 ~ /^(add|sub|mul|div)$/) ? 3 : 2
    t[k] += $(off + 1); b[k] += $(off + 2)
    if (!(k in seen)) { seen[k] = 1; ord[++n] = k } }
  END { for (i = 1; i <= n; i++) { k = ord[i]; printf "%-24s n=%-6d bad=%d\n", k, t[k], b[k]; bad += b[k] }
        printf "f64 N=%d per set, mismatches %d\n", n_per_set, bad; exit bad > 0 }' $T/all.txt
echo "llrm-cc $(md5sum $LLRM_CC | cut -c1-12) -$OPT"
