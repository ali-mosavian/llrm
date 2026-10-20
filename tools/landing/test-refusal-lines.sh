#!/usr/bin/env bash
# Regression: land.sh printed `N: REFUSED: -` for every landing that succeeded (the batch summary line says `LANDED: #N  REFUSED: -`); owners read it as a refusal.
source "$(dirname "$0")/refusal-lines.sh"
d=$(mktemp -d)
printf 'STAGE squash #7\nLANDED: #7  REFUSED: -\n' > $d/ok.log
printf '#8: left out: conflicts\nLANDED: -  REFUSED: #8\n' > $d/bad.log
[ -z "$(refusal_lines $d/ok.log)" ] || { echo "FAIL: a landing that succeeded reads as refused: '$(refusal_lines $d/ok.log)'"; exit 1; }
[ -n "$(refusal_lines $d/bad.log)" ] && echo PASS || { echo "FAIL: a real refusal is dropped"; exit 1; }
# Regression: the checkout lock lived under $LAND_TMP (/tmp/landing for owners, the session tmp dir for the runner): two different files, so a land.sh and the runner reset the same checkout at once. It is next to the checkout now.
cd "$(dirname "$0")" && [ "$(grep -c '9>"$W.checkout.lock"' land.sh queue-runner.sh | tr '\n' ' ')" = "land.sh:1 queue-runner.sh:1 " ] && echo PASS-lock || { echo "FAIL: checkout lock not beside the checkout"; exit 1; }
