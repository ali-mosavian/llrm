#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc> LLRM_CLANG_M16=... tests/small_copy_inline_test.sh
# CVI/CVL/CVS/CVD/MKI/MKL/MKS/MKD copied 2..8 bytes through the far B$COPY call (CVI 49 instructions, BC's 2): a copy of a constant few bytes is loads and stores.
cd "$(dirname "$0")/.." || exit 2
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
LLRM_CC_TARGET=x86_16-wat "${LLRM_CC_DIR:?dir with llrm-cc}/llrm-cc" -march=i386 -O2 -S -I. -Idos/m16 -I../shared strconv.c -o $T/s.s || exit 2
# strconv.c keeps one B$COPY call, the variable-length one in HEX$/OCT$ (digits); the fixed 2..8 byte copies are inline
n=$(grep -c 'call far ptr B\$COPY' $T/s.s)
[ "$n" -le 1 ] && echo "small_copy_inline ok" || { echo "FAIL: strconv.c calls B\$COPY $n times (the constant-size copies in CV*/MK* are 5)"; exit 1; }
