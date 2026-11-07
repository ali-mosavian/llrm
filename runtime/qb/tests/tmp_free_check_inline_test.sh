#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc> LLRM_OW=... LLRM_CLANG_M16=... tests/tmp_free_check_inline_test.sh
# 130,816 of 327,556 str_tmp_free calls on stft_fixed were for a variable, not a temporary: a far call and a frame (11-13 instructions) to find that out.
# The test of "is this descriptor a temporary" is in line (str_free_if_tmp): B$FASC compares against the temporaries before it calls.
cd "$(dirname "$0")/.." || exit 2
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
LLRM_CC_TARGET=x86_16-wat "${LLRM_CC_DIR:?dir with llrm-cc}/llrm-cc" -march=i386 -O2 -S -I. -Idos/m16 -I../shared strconv.c -o $T/s.s 2> /dev/null || exit 2
awk '/^B\$FASC proc/ {on=1} on && /offset _str_temps/ {t=1} on && /call far ptr _str_tmp_free@3/ {if (!t) bad=1; c++} /^B\$FASC endp/ {on=0} END {exit !(c > 0 && !bad)}' $T/s.s && echo "tmp_free_check_inline ok" || { echo "FAIL: B\$FASC calls str_tmp_free without testing for a temporary first"; exit 1; }
