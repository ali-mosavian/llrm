#!/bin/sh
# fn_diff.py once compared only a function's first lines: a difference after the first inner label (past line 86) read as identical, and the 6502 +0.7% was blamed on the wrong function.
set -e
T=$(mktemp -d); cd "$(dirname "$0")"
mk() { { echo 'rc0 = $80'; echo '.export f'; echo 'f:'; i=0; while [ $i -lt 100 ]; do echo '  nop'; i=$((i+1)); done; echo 'f_b1:'; echo "  $1"; echo '  rts'; } > $2; }
mk lda $T/a.s; mk ldx $T/b.s; mk lda $T/c.s
[ "$(python3 fn_diff.py $T/a.s -- $T/b.s)" = "f differs 104 104" ] || { echo "FAIL: late difference missed"; exit 1; }
[ -z "$(python3 fn_diff.py $T/a.s -- $T/c.s)" ] || { echo "FAIL: identical files differ"; exit 1; }
echo ok
