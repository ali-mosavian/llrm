#!/usr/bin/env bash
# Regression: land.sh printed `N: REFUSED: -` for every landing that succeeded (the batch summary line says `LANDED: #N  REFUSED: -`); owners read it as a refusal.
source "$(dirname "$0")/refusal-lines.sh"
d=$(mktemp -d)
printf 'STAGE squash #7\nLANDED: #7  REFUSED: -\n' > $d/ok.log
printf '#8: left out: conflicts\nLANDED: -  REFUSED: #8\n' > $d/bad.log
[ -z "$(refusal_lines $d/ok.log)" ] || { echo "FAIL: a landing that succeeded reads as refused: '$(refusal_lines $d/ok.log)'"; exit 1; }
[ -n "$(refusal_lines $d/bad.log)" ] && echo PASS || { echo "FAIL: a real refusal is dropped"; exit 1; }
