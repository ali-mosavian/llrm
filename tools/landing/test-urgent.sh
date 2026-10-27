#!/usr/bin/env bash
# Regression: landing fixes (the tooling PRs) waited behind a 10-PR batch that the fixes themselves would have kept from failing; PRs listed in $LAND_TMP/urgent.txt form their own small batch.
here=$(cd "$(dirname "$0")" && pwd); source "$here/urgent.sh"
f=$(mktemp); trap 'rm -f $f' EXIT; printf '# fixes\n5073 5125 # docs\n5123\n' > $f
[ "$(urgent_listed $f 1 5123 2 5073 3 | tr '\n' ' ')" = "5073 5123 " ] || { echo "FAIL: listed PRs of the candidates, in the file's order: '$(urgent_listed $f 1 5123 2 5073 3 | tr '\n' ' ')'"; exit 1; }
[ -z "$(urgent_listed $f 1 2 3)" ] || { echo "FAIL: an unlisted candidate was urgent"; exit 1; }
[ -z "$(urgent_listed /nonexistent 5073)" ] || { echo "FAIL: a missing file made a PR urgent"; exit 1; }
[ -z "$(urgent_listed $f 50731 507)" ] || { echo "FAIL: a prefix made a PR urgent"; exit 1; }
grep -q 'urgent_listed' "$here/queue-runner.sh" || { echo "FAIL: queue-runner does not use the urgent list"; exit 1; }
echo PASS urgent
