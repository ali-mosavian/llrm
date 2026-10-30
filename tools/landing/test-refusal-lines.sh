#!/usr/bin/env bash
# Regression: land.sh printed `N: REFUSED: -` for every landing that succeeded (the batch summary line says `LANDED: #N  REFUSED: -`); owners read it as a refusal.
here=$(cd "$(dirname "$0")" && pwd)
source "$here/refusal-lines.sh"
d=$(mktemp -d)
printf 'STAGE squash #7\nLANDED: #7  REFUSED: -\n' > $d/ok.log
printf '#8: left out: conflicts\nLANDED: -  REFUSED: #8\n' > $d/bad.log
[ -z "$(refusal_lines $d/ok.log)" ] || { echo "FAIL: a landing that succeeded reads as refused: '$(refusal_lines $d/ok.log)'"; exit 1; }
# Regression: #3344 was refused five times and the reply read `3344: REFUSED: #3344` with no reason (the reason sits on the `left out:` line).
case "$(refusal_lines $d/bad.log)" in *"left out: conflicts"*) ;; *) echo "FAIL: the reason of a refusal is not printed: '$(refusal_lines $d/bad.log)'"; exit 1;; esac
[ -n "$(refusal_lines $d/bad.log)" ] && echo PASS || { echo "FAIL: a real refusal is dropped"; exit 1; }
# Regression: the checkout lock lived under $LAND_TMP (/tmp/landing for owners, the session tmp dir for the runner): two different files, so a land.sh and the runner reset the same checkout at once. It is next to the checkout now.
cd "$here" && [ "$(grep -c '9>"$W.checkout.lock"' land.sh queue-runner.sh | tr '\n' ' ')" = "land.sh:1 queue-runner.sh:1 " ] && echo PASS-lock || { echo "FAIL: checkout lock not beside the checkout"; exit 1; }
# Regression: a land.sh blocked on the checkout lock printed nothing (it read as a failed run, two sessions re-ran it): it says it is waiting.
cd "$here" && [ "$(grep -c 'waiting for the landing lock' land.sh queue-runner.sh | tr '\n' ' ')" = "land.sh:1 queue-runner.sh:1 " ] && echo PASS-wait || { echo "FAIL: no waiting message"; exit 1; }
# Regression: a batch that crashed (the 05:18 TypeError in docs_gated) printed no REFUSED line, and the runner reported `N: REFUSED: batch LOG: ` with an empty reason for all 13 PRs; it names the log tail.
source "$here/refusal-lines.sh"
printf '05:18:23 STAGE fast-merge\n05:18:23 Traceback (most recent call last):\n05:18:23 TypeError: docs_stale() missing 1 required positional argument: '"'root'"'\n' > $d/crash.log
case "$(batch_refusal_reason $d/crash.log 5082)" in *"TypeError: docs_stale()"*) echo PASS-crash;; *) echo "FAIL: a crashed batch's reason is blank: '$(batch_refusal_reason $d/crash.log 5082)'"; exit 1;; esac
printf '05:10:00 #7: REFUSED: left out: conflicts\n' > $d/named.log
case "$(batch_refusal_reason $d/named.log 7)" in "#7: REFUSED: left out: conflicts"*|*"REFUSED: left out: conflicts"*) echo PASS-named;; *) echo "FAIL: a named refusal lost its reason"; exit 1;; esac
grep -q 'batch_refusal_reason' "$here/queue-runner.sh" || { echo "FAIL: queue-runner does not use batch_refusal_reason"; exit 1; }

# Regression: the two PRs of each one-push batch (c8b4d4e8dd, be7e11f7d0) were reported `REFUSED: ... PUSHED` because GitHub marks them MERGED a few seconds after the push.
d=$(mktemp -d); printf '#!/bin/sh\nn=$(cat %s/n 2>/dev/null || echo 0); echo $((n+1)) > %s/n\n[ "$n" -ge 2 ] && echo MERGED || echo OPEN\n' $d $d > $d/gh; chmod +x $d/gh
printf '02:47:09 PUSHED: abc to release/next with #7 #8\n' > $d/pushed.log
PATH=$d:$PATH PUSH_SETTLE=0 bash -c 'source "$1/refusal-lines.sh"; [ "$(landed_state $2/pushed.log 7)" = MERGED ]' _ "$here" "$d" && echo PASS-pushed || { echo "FAIL: a pushed PR read as not merged"; exit 1; }
rm -f $d/n; printf 'LANDED: -  REFUSED: #9\n' > $d/none.log
[ "$(PATH=$d:$PATH PUSH_SETTLE=0 bash -c 'source "$1/refusal-lines.sh"; landed_state $2/none.log 9' _ "$here" "$d")" = OPEN ] && [ "$(cat $d/n)" = 1 ] && echo PASS-no-wait || { echo "FAIL: a PR not pushed was waited for"; exit 1; }
