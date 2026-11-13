#!/bin/bash
# usage: tests/z80_lib_ident.sh BASE_LLC FIX_LLC -> the soft-float library built with each compiler, member by member: `DIFF member bytes -> bytes` for each
# whose text or _CODE size differs, `same: N members` otherwise. A program-asm identity run cannot see a change that lives in the library (2026-11-13: the
# f64 divide fix changed no program and one member, +14 B). Exits 1 on a difference, so a claimed "no change" fails here.
set -e
cd "$(dirname "$0")/.."
BASE=${1:?base llrm-z80-llc}; FIX=${2:?fixed llrm-z80-llc}
T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_Z80_LLC=$BASE bash tests/z80_lib.sh $T/base > /dev/null
LLRM_Z80_LLC=$FIX bash tests/z80_lib.sh $T/fix > /dev/null
code() { awk '$1=="A" && $2=="_CODE" {print $4; exit}' "$1"; }
diffs=0; members=0
for f in $T/fix/*.s; do
  m=$(basename $f .s); members=$((members + 1))
  cmp -s $f $T/base/$m.s && continue
  diffs=$((diffs + 1)); echo "DIFF $m $((16#$(code $T/base/$m.rel))) -> $((16#$(code $T/fix/$m.rel))) B"
done
[ $diffs -eq 0 ] && echo "same: $members members" || exit 1
