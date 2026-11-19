#!/usr/bin/env bash
# Regression: the queue-runner log held the same REFUSED line for each PR on every tick (12.6k lines); readers got the repeats, not the current reason.
here=$(cd "$(dirname "$0")" && pwd)
source "$here/last-refusals.sh"
d=$(mktemp -d)
printf '7: REFUSED: conflicts\n8: REFUSED: docs out of date\n7: REFUSED: conflicts\n7: REFUSED: fmt\n== 10:00 landing 9\n' > $d/q.log
[ "$(last_refusals $d/q.log 7)" = "7: REFUSED: fmt" ] || { echo "FAIL: not the last line for 7: $(last_refusals $d/q.log 7)"; exit 1; }
[ "$(last_refusals $d/q.log 7 8 9 | wc -l)" = 3 ] || { echo "FAIL: not one line per PR"; exit 1; }
[ "$(last_refusals $d/q.log 9)" = "9: none" ] || { echo "FAIL: a PR never refused"; exit 1; }
[ "$(last_refusals $d/q.log 70)" = "70: none" ] || { echo "FAIL: 70 matched 7's line"; exit 1; }
echo PASS
