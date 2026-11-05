#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc tests/6502_members.sh OUTDIR [GROUP...]   (default groups: int64 f64 f32 f32_64)
# Builds the runtime as per-routine 6502 members for a resolver (llrm-target-6502's reachable_runtime): OUTDIR/members/<dir>_<file>.s, one per source file, and
# OUTDIR/index.tsv "symbol<TAB>member.s<TAB>needs" (needs = other index symbols the member's assembly names, comma separated). Each member exports its index
# symbols only (llrm-cc exports no statics; 6502_members_test.sh guards that). The rc* equates are repeated in every member; the resolver collects them once.
set -e
cd "$(dirname "$0")/.."; LLRM_CC=${LLRM_CC:?set LLRM_CC}; OUT=${1:?OUTDIR}; shift
G=${*:-int64 f64 f32 f32_64}
mkdir -p "$OUT/members"; rm -f "$OUT"/members/*.s; T=$(mktemp -d); trap 'rm -r $T' EXIT
RT_CFLAGS="-include stdint.h -include config/6502/config.h" RT_EXTRA=config/6502/sf32_6502.c sh tests/rt_index.sh $G > $T/host.tsv   # symbol, source file (names as the C defines them)
[ -s $T/host.tsv ] || { echo "6502_members: empty index" >&2; exit 2; }
cut -f1 $T/host.tsv | sort -u > $T/syms
: > $T/index
for f in $(cut -f2 $T/host.tsv | sort -u); do
  m=$(basename $(dirname $f))_$(basename $f .c).s
  $LLRM_CC --target=6502-sim65 -Os -S -DSF_INTABI -include config/6502/config.h -Iinclude $f -o $T/raw.s
  cp $T/raw.s "$OUT/members/$m"
  for s in $(awk -F'\t' -v f=$f '$2 == f { print $1 }' $T/host.tsv); do
    grep -q "^\.export $s\$" "$OUT/members/$m" || { echo "6502_members: $m does not export $s" >&2; exit 1; }
    printf '%s\t%s\n' "$s" "members/$m" >> $T/index
  done
done
# needs: index symbols named in a member's assembly (not its own)
while IFS="$(printf '\t')" read -r sym m; do
  own=$(awk -F'\t' -v m=$m '$2 == m { print $1 }' $T/index | tr '\n' ' ')
  need=$(grep -v '^\.export ' "$OUT/$m" | grep -oE '[A-Za-z_][A-Za-z0-9_]*' | sort -u | grep -Fxf $T/syms | while read -r x; do case " $own " in *" $x "*) ;; *) echo $x;; esac; done | tr '\n' ',' | sed 's/,$//')
  printf '%s\t%s\t%s\n' "$sym" "$m" "$need"
done < $T/index | sort > "$OUT/index.tsv"
echo "6502_members: $(wc -l < "$OUT/index.tsv") symbols in $(ls "$OUT/members" | wc -l) members -> $OUT/index.tsv"
