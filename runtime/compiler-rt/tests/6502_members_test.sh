#!/bin/sh
# usage: LLRM_CC=llrm-cc tests/6502_members_test.sh -> the int64 members for a 6502 resolver: a shift pulls no divide, the divides need the shared worker, no member exports a
# symbol that is not in the index (llrm-cc exports statics, which collide when members are linked together), and a compiler that fails stops the build.
set -e
cd "$(dirname "$0")/.."; LLRM_CC=${LLRM_CC:?set LLRM_CC}; T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_CC=$LLRM_CC sh tests/6502_members.sh $T/o int64 f32 > /dev/null
awk -F'\t' '$1 == "__ashldi3" && $3 == "" && $2 != ""' $T/o/index.tsv | grep -q . || { echo "FAIL: __ashldi3 should be a member with no needs"; exit 1; }
awk -F'\t' '$1 == "__divdi3" && $3 == "__udivmod_w"' $T/o/index.tsv | grep -q . || { echo "FAIL: __divdi3 should need exactly __udivmod_w"; exit 1; }
cut -f1 $T/o/index.tsv | sort -u > $T/syms
for m in $T/o/members/*.s; do
  for s in $(sed -n 's/^\.export \([^ \t]*\).*/\1/p' $m); do
    grep -qxF "$s" $T/syms || { echo "FAIL: $(basename $m) exports $s, not an index symbol"; exit 1; }
  done
done
printf '#!/bin/sh\nexit 1\n' > $T/bad; chmod +x $T/bad
if LLRM_CC=$T/bad sh tests/6502_members.sh $T/o2 int64 f32 > /dev/null 2>&1; then echo "FAIL: a failing compiler did not stop 6502_members.sh"; exit 1; fi
echo "6502_members ok"
