#!/usr/bin/env bash
# Regression: #2836 failed a ratchet in the merged tree and cost a whole batch 26 min of halving. A PR whose precheck refuses never reaches a batch; the refusal line reaches the log.
here=$(cd "$(dirname "$0")" && pwd); source "$here/precheck-filter.sh"; d=$(mktemp -d)
printf '#!/bin/sh\ncase "$1" in 2) echo "2: REFUSED: ratchet:facts_rewrite (x)";; 4) ;; *) echo "$1: PASS";; esac\n' > $d/pc; chmod +x $d/pc
export PRECHECK_CMD=$d/pc
out=$(prechecked 1 2 3 4 5 2>$d/err | tr '\n' ' ')
[ "$out" = "1 3 5 " ] || { echo "FAIL: passed set '$out', want '1 3 5 '"; exit 1; }
grep -q "^2: REFUSED: ratchet:facts_rewrite" $d/err || { echo "FAIL: the refusal did not reach stderr"; exit 1; }
grep -q "^4: REFUSED: precheck printed nothing" $d/err || { echo "FAIL: a silent precheck passed"; exit 1; }
echo PASS
