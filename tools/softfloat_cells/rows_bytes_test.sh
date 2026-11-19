#!/bin/bash
# usage: LLRM_CC_DIR=dir tools/softfloat_cells/rows_bytes_test.sh
# rows_bytes.sh m32 ranked neg as a gap (ours 14 B vs compiler-rt 7): compiler-rt returns floats in st0 (`flds; fchs`), ours in integer registers.
# Starred cells also get an ours+x87 column (ours plus the 17 / 15 B st0 return shim, measured with clang) to rank against.
# A float-returning op's compiler-rt cell must carry the `*` mark (not a rank); libgcc is built -msoft-float: neg 21 B, not the 34 of the x87 return.
cd "$(dirname "$0")" || exit 2
out=$(ROWS_ONLY=neg bash ./rows_bytes.sh m32) || exit 2; bad=0
echo "$out" | awk '$1=="neg"' | while read -r n o6 c6 g6 bar o3 c3 g3; do
  case "$c6$c3" in *"*"*"*"*) ;; *) echo "FAIL: neg compiler-rt cells '$c6' '$c3' lack the x87-return mark"; exit 1;; esac
done || bad=1
echo "$out" | awk '$1=="neg"' | head -1 | awk '{ if ($4 >= 34) { print "FAIL: libgcc neg " $4 " B is the x87 return"; exit 1 } }' || bad=1
# like-for-like: ours + the x87 return shim (neg: 14 + 17, 10 + 15) in the last two columns
echo "$out" | awk '$1=="neg"' | head -1 | awk '{ if ($(NF-1) != $2 + 17 || $NF != $6 + 15) { print "FAIL: neg ours+x87 columns " $(NF-1) " " $NF " (want ours64+17, ours32+15)"; exit 1 } }' || bad=1
# compiler-rt's m32 int->float is x87 code (fild, fmul, fadd): 290 B read as a soft-float row ours lost by 22 B; libgcc's soft-fp (954 B) is the comparator
out=$(ROWS_ONLY=float bash ./rows_bytes.sh m32) || exit 2
echo "$out" | awk '$1=="float"' | head -1 | awk '{ if ($3 != "fpu" || $7 != "fpu") { print "FAIL: float compiler-rt cells " $3 " " $7 " are x87 code, not a rank"; exit 1 } }' || bad=1
# wasm32: compiler-rt's f64 int64->float is f64.convert/mul/add (276 B read as a soft-float row ours "lost" 259 by 17); the f32 cell is bitwise and stays a rank
out=$(ROWS_ONLY=float bash ./rows_bytes.sh wasm32) || exit 2
echo "$out" | awk '$1=="float"' | head -1 | awk '{ if ($3 != "fpu" || $7 == "fpu") { print "FAIL: wasm32 float compiler-rt cells " $3 " " $7 " (want f64 fpu, f32 a number)"; exit 1 } }' || bad=1
# cmp: compiler-rt's compare object holds __unord*f2 as well, so ours is cmp.c plus unord.c (wasm32 -Os f32: 185 B read as the whole set, 220 with unord)
out=$(ROWS_ONLY=cmp bash ./rows_bytes.sh wasm32) || exit 2
echo "$out" | awk '$1=="cmp"' | head -1 | awk '{ if ($6 < 200) { print "FAIL: wasm32 -Os f32 cmp ours " $6 " B leaves out unord.c"; exit 1 } }' || bad=1
# gcc -O2 pads loops, jumps and labels with .p2align inside .text: libgcc fix_usi -O2 read 107 (f32) / 139 (f64) B, 88 / 113 unpadded; the column must count code, not nops
out=$(ROWS_ONLY=fix_usi bash ./rows_bytes.sh m32) || exit 2
echo "$out" | awk '$1=="fix_usi"' | tail -1 | awk '{ if ($4 > 113 || $8 > 88) { print "FAIL: m32 -O2 libgcc fix_usi " $4 " | " $8 " B include alignment padding (want 113 | 88)"; exit 1 } }' || bad=1
# libgcc has no comparedf2/comparesf2: the cmp libgcc cells read ERR, so a worse-than-gcc compare could not show (eq+le+ge+unord: 790 | 530 B at -Os)
out=$(ROWS_ONLY=cmp bash ./rows_bytes.sh m32) || exit 2
echo "$out" | awk '$1=="cmp"' | head -1 | awk '{ if ($4 == "ERR" || $8 == "ERR") { print "FAIL: m32 cmp libgcc cells " $4 " | " $8; exit 1 } }' || bad=1
[ $bad = 0 ] && echo "rows_bytes ok"; exit $bad
