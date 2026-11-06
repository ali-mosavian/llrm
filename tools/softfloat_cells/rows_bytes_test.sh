#!/bin/bash
# usage: LLRM_CC_DIR=dir LLRM_OW=dir tools/softfloat_cells/rows_bytes_test.sh
# rows_bytes.sh m32 ranked neg as a gap (ours 14 B vs compiler-rt 7): compiler-rt returns floats in st0 (`flds; fchs`), ours in integer registers.
# A float-returning op's compiler-rt cell must carry the `*` mark (not a rank); libgcc is built -msoft-float: neg 21 B, not the 34 of the x87 return.
cd "$(dirname "$0")" || exit 2
out=$(ROWS_ONLY=neg bash ./rows_bytes.sh m32) || exit 2; bad=0
echo "$out" | awk '$1=="neg"' | while read -r n o6 c6 g6 bar o3 c3 g3; do
  case "$c6$c3" in *"*"*"*"*) ;; *) echo "FAIL: neg compiler-rt cells '$c6' '$c3' lack the x87-return mark"; exit 1;; esac
done || bad=1
echo "$out" | awk '$1=="neg"' | head -1 | awk '{ if ($4 >= 34) { print "FAIL: libgcc neg " $4 " B is the x87 return"; exit 1 } }' || bad=1
[ $bad = 0 ] && echo "rows_bytes ok"; exit $bad
