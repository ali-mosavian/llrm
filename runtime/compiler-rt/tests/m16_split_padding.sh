#!/bin/bash
# usage: [SF_FLAGS=-DSF_CMP_SPLIT] tests/m16_split_padding.sh [OPT] -> x86_16-wat -march=i486: per-object code bytes of the split f32 files, the linked bytes reached
# from each entry (jwlink, option eliminate) and the linked CODE span (reached + alignment padding between segments), against a control:
# the same sources in one translation unit. Needs LLRM_CC_DIR (llrm-cc), LLRM_OW, LLRM_CLANG_M16 as size_m16.sh.
set -e
cd "$(dirname "$0")/.."
OPT=${1:-Os}; D=${LLRM_CC_DIR:-$HOME/targets/gs-out/rbase}; T=$(mktemp -d); trap 'rm -r $T' EXIT; export LLRM_CC_TARGET=x86_16-wat
CFG="-include config/x86_m16/config.h $SF_FLAGS"
cc1() { $D/llrm-cc -march=i486 -$OPT $CFG -Iinclude -c "$@"; }
SRC="$(tests/fp_sources.sh f32) lib/int/clzsi2.c"
mkdir $T/split $T/flat
for p in $SRC; do cc1 $p -o $T/split/$(basename $p .c).obj; done
{ for p in $(tests/fp_sources.sh f32); do python3 tests/flatten.py $p; done; case $OPT in Os) python3 tests/flatten.py lib/int/clzsi2.c;; esac; } > $T/flat/all.c
cc1 $T/flat/all.c -o $T/flat/all.obj
tot() { for o in "$@"; do python3 tests/omf_code_bytes.py $o; done | paste -sd+ | bc; }
echo "== -$OPT objects: split $(tot $T/split/*.obj) B, flat $(tot $T/flat/all.obj) B"
link() { # DIR ENTRY -> reached CODE bytes, CODE span
  (cd $T/$1; { echo "format dos"; echo "name t.exe"; echo "option start='$2'"; echo "option map=t.map"; echo "option eliminate"; for f in *.obj; do echo "file $f"; done; } > t.lnk
   jwlink @t.lnk > t.log 2>&1 || { echo "link failed: $(grep -m1 -i error t.log)"; exit 1; }
   python3 - <<'P'
import re
segs = []
for l in open('t.map'):
    m = re.match(r'^(\S+)\s+CODE\s+\S+\s+([0-9a-fA-F]+):([0-9a-fA-F]+)\s+([0-9a-fA-F]+)', l)
    if m: segs.append((int(m.group(2), 16) * 16 + int(m.group(3), 16), int(m.group(4), 16)))
sz = sum(s for _, s in segs); span = max(a + s for a, s in segs) - min(a for a, _ in segs) if segs else 0
print(sz, span)
P
  )
}
printf "%-14s %14s %14s\n" entry "split reach/span" "flat reach/span"
for e in __addsf3 __subsf3 __mulsf3 __divsf3 __negsf2 __ltsf2 __fixsfsi __fixunssfsi __floatsisf __floatunsisf; do
  printf "%-14s %14s %14s\n" $e "$(link split ___${e#__}@3 2>&1 | tr '\n' ' ')" "$(link flat ___${e#__}@3 2>&1 | tr '\n' ' ')"
done
