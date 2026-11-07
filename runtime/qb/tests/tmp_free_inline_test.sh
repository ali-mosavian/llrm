#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc> LLRM_CLANG_M16=... tests/tmp_free_inline_test.sh
# Every freed string temporary cost two far calls (str_tmp_free -> str_release): 11 + 19 instructions plus the second frame, 196,740 times on stft_fixed.
# The release is the hot path of str_tmp_free (a loop's temporaries), so it is inline there.
cd "$(dirname "$0")/.." || exit 2
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
LLRM_CC_TARGET=x86_16-wat "${LLRM_CC_DIR:?dir with llrm-cc}/llrm-cc" -march=i386 -O2 -S -I. -Idos/m16 -I../shared nhstutil.c -o $T/s.s 2> /dev/null || exit 2
n=$(awk '/^_str_tmp_free@3 proc/ {on=1} on && /call far ptr _str_release/ {c++} /endp/ {on=0} END {print c+0}' $T/s.s)
[ "$n" = 0 ] && echo "tmp_free_inline ok" || { echo "FAIL: str_tmp_free calls str_release ($n)"; exit 1; }
