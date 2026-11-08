#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc> LLRM_CLANG_M16=... tests/str_alloc_size_test.sh
# str_alloc ran 46 instructions a call on stft_fixed (BC's STALC about 34): take() re-derived the entry size for an owned entry that never reaches it,
# which spilled EVEN(len) to a frame (enter/leave). Free-entry take makes it 60 static instructions at -O2 m16 and the frame goes (80 before).
cd "$(dirname "$0")/.." || exit 2
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
LLRM_CC_TARGET=x86_16-wat "${LLRM_CC_DIR:?dir with llrm-cc}/llrm-cc" -march=i386 -O2 -S -I. -Idos/m16 -I../shared nhstutil.c -o $T/s.s 2> /dev/null || exit 2
n=$(awk '/^_str_alloc@3 proc/ {on=1} on && /^    / {n++} /endp/ {on=0} END {print n+0}' $T/s.s)
f=$(awk '/^_str_alloc@3 proc/ {on=1} on && /enter|leave/ {n++} /endp/ {on=0} END {print n+0}' $T/s.s)
[ "$n" -le 65 ] && [ "$f" = 0 ] && echo "str_alloc_size ok ($n, no frame)" || { echo "FAIL: str_alloc is $n instructions with $f frame ops (bound 65, none)"; exit 1; }
