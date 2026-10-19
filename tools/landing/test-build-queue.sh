#!/usr/bin/env bash
# Regression: the hand-built queue dropped 17 of 58 ready PRs, and `gh pr list` without --limit silently returned 30. A stub gh returns only 30 PRs unless
# given --limit >= the count. The queue must hold every ready PR, minus waits, priority first, even those missing from priority.txt.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d)
cat > $d/gh <<'S'
#!/bin/sh
limit=30; q=.; while [ $# -gt 0 ]; do case "$1" in --limit) limit=$2;; -q) q=$2;; esac; shift; done
i=0; while [ $i -lt 40 ]; do n=$((3000+i)); d=false; [ $i -eq 5 ] && d=true; m=MERGEABLE; [ $i -eq 7 ] && m=CONFLICTING; echo "{\"number\":$n,\"isDraft\":$d,\"mergeable\":\"$m\"}"; i=$((i+1)); done | head -n $limit | jq -s "$q" | jq -r 'if type=="array" then .[] else . end'
S
chmod +x $d/gh; echo "3030 3002" > $d/priority.txt; echo "3010 # waits" > $d/waits.txt
out=$(PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>$d/err | tr '\n' ' ')
grep -q "^3007: REFUSED: conflicts with feat/parity" $d/err || { echo "FAIL: a conflicting PR was queued or not reported"; exit 1; }
want="3030 3002 $(seq 3000 3039 | grep -vxE '3030|3002|3005|3007|3010' | tr '\n' ' ')"
[ "$out" = "$want" ] && echo PASS || { echo "FAIL"; echo " got: $out"; echo "want: $want"; exit 1; }
