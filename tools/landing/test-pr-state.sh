#!/usr/bin/env bash
# Regression: every stage (open, age, base) asked GitHub per PR, about 100 REST calls a cycle, and the quota ran out. After one fill, open_only and aged answer from it with no gh call; an old fill falls back to gh.
here=$(cd "$(dirname "$0")" && pwd); HERE=$here; source "$here/open-only.sh"; source "$here/head-age.sh"; d=$(mktemp -d)
export LAND_TMP=$d; log=$d/gh.log
printf '#!/bin/sh\necho "$@" >> %s\necho OPEN\n' "$log" > $d/gh; chmod +x $d/gh; PATH=$d:$PATH
old=$(date -u -d '30 minutes ago' +%FT%TZ)
printf '{"fetched": %s, "prs": {"1": {"state": "OPEN", "committedDate": "%s"}, "2": {"state": "CLOSED", "committedDate": "%s"}}}' "$(date +%s)" "$old" "$old" > $d/pr-state.json
out=$(open_only 1 2 2>/dev/null | tr '\n' ' '); [ "$out" = "1 " ] || { echo "FAIL: open_only from the fill: '$out'"; exit 1; }
out=$(aged 1 2>/dev/null | tr '\n' ' '); [ "$out" = "1 " ] || { echo "FAIL: aged from the fill: '$out'"; exit 1; }
[ ! -s "$log" ] || { echo "FAIL: gh was called with a fresh fill: $(cat $log)"; exit 1; }
printf '{"fetched": %s, "prs": {"1": {"state": "CLOSED"}}}' "$(( $(date +%s) - 600 ))" > $d/pr-state.json
out=$(open_only 1 2>/dev/null | tr '\n' ' '); [ "$out" = "1 " ] && [ -s "$log" ] || { echo "FAIL: an old fill should fall back to gh (state OPEN): '$out'"; exit 1; }
echo PASS
