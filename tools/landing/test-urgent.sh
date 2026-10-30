#!/usr/bin/env bash
# Regression: landing fixes (the tooling PRs) waited behind a 10-PR batch that the fixes themselves would have kept from failing; PRs listed in $LAND_TMP/urgent.txt form their own small batch.
here=$(cd "$(dirname "$0")" && pwd); source "$here/urgent.sh"
f=$(mktemp); trap 'rm -f $f' EXIT; printf '# fixes\n5073 5125 # docs\n5123\n' > $f
[ "$(urgent_listed $f 1 5123 2 5073 3 | tr '\n' ' ')" = "5073 5123 " ] || { echo "FAIL: listed PRs of the candidates, in the file's order: '$(urgent_listed $f 1 5123 2 5073 3 | tr '\n' ' ')'"; exit 1; }
[ -z "$(urgent_listed $f 1 2 3)" ] || { echo "FAIL: an unlisted candidate was urgent"; exit 1; }
[ -z "$(urgent_listed /nonexistent 5073)" ] || { echo "FAIL: a missing file made a PR urgent"; exit 1; }
[ -z "$(urgent_listed $f 50731 507)" ] || { echo "FAIL: a prefix made a PR urgent"; exit 1; }
grep -q 'urgent_from_queue\|urgent_listed' "$here/queue-runner.sh" || { echo "FAIL: queue-runner does not use the urgent list"; exit 1; }
# Regression: sqb 22:06 (32 PRs) left out urgent #5558 and #5561, at lines 59 and 62 of the queue, beyond the window of BATCH_CAP+24; the batch failed on the red parity tip they fix.
q=$(mktemp); (seq 5400 5457; echo 5558; seq 5458 5460; echo 5561) > $q; printf '5561 5558\n' > $f
[ "$(urgent_from_queue $f $q | tr '\n' ' ')" = "5561 5558 " ] || { echo "FAIL: an urgent PR beyond the window was not taken: '$(urgent_from_queue $f $q | tr '\n' ' ')'"; exit 1; }
[ -z "$(urgent_from_queue $f /nonexistent)" ] || { echo "FAIL: a missing queue made a PR urgent"; exit 1; }
grep -q 'urgent_from_queue' "$here/queue-runner.sh" || { echo "FAIL: queue-runner reads the urgent list through the window"; exit 1; }
rm -f $q
grep -q 'urgent: ' "$here/queue-runner.sh" || { echo "FAIL: the assembly does not log its urgent list (the 22:06 batch left two out and nothing said why)"; exit 1; }
grep -q "build-queue \$(( " "$here/queue-runner.sh" || { echo "FAIL: the pass does not log build-queue's duration"; exit 1; }
echo PASS urgent
