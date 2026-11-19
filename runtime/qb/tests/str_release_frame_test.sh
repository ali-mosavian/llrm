#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc> LLRM_CLANG_M16=... tests/str_release_frame_test.sh
# str_tmp_free ran 4.7M instructions on stft_fixed, each with enter/leave: the owner-check error call kept the descriptor live across a call (m16 calls destroy bx),
# which spilled it to a frame. The error path is out of line now and tmp_release runs first, so nothing is live across it: 31 -> 28 instructions, no frame.
cd "$(dirname "$0")/.." || exit 2
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
LLRM_CC_TARGET=x86_16-wat "${LLRM_CC_DIR:?dir with llrm-cc}/llrm-cc" -march=i386 -O2 -S -I. -Idos/m16 -I../shared nhstutil.c -o $T/s.s 2> /dev/null || exit 2
bad=0
for f in str_tmp_free str_release; do
  fr=$(awk -v f="^_${f}@3 proc" '$0 ~ f {on=1} on && /^    (enter|leave)/ {n++} /endp/ {on=0} END {print n+0}' $T/s.s)
  [ "$fr" = 0 ] || { echo "FAIL: $f has $fr frame instructions"; bad=1; }
done
n=$(awk '/^_str_release@3 proc/ {on=1} on && /^    / {n++} /endp/ {on=0} END {print n+0}' $T/s.s)
[ "$n" -le 26 ] || { echo "FAIL: str_release is $n instructions (bound 26)"; bad=1; }
[ $bad = 0 ] && echo "str_release_frame ok"; exit $bad
