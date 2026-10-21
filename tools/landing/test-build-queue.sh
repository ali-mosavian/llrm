#!/usr/bin/env bash
# Regression: the hand-built queue dropped 17 of 58 ready PRs, and `gh pr list` without --limit silently returned 30. A stub gh returns only 30 PRs unless
# given --limit >= the count. The queue must hold every ready PR, minus waits, priority first, even those missing from priority.txt.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d)
cat > $d/gh <<'S'
#!/bin/sh
[ "$1" = api ] && { echo paritysha1; exit 0; }
limit=30; q=.; while [ $# -gt 0 ]; do case "$1" in --limit) limit=$2;; -q) q=$2;; esac; shift; done
i=0; while [ $i -lt 40 ]; do n=$((3000+i)); d=false; [ $i -eq 5 ] && d=true; m=MERGEABLE; [ $i -eq 7 ] && m=CONFLICTING; b=""; case $i in 2|30) ;; *) b="Owner: sess-$i";; esac; echo "{\"number\":$n,\"isDraft\":$d,\"mergeable\":\"$m\",\"body\":\"$b\",\"headRefOid\":\"h$n\"}"; i=$((i+1)); done | head -n $limit | jq -s "$q" | jq -r 'if type=="array" then .[] else . end'
S
chmod +x $d/gh; echo "3030 3002" > $d/priority.txt; echo "3010 # waits" > $d/waits.txt
out=$(PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>$d/err | tr '\n' ' ')
grep -q "^3007: REFUSED: conflicts with feat/parity" $d/err || { echo "FAIL: a conflicting PR was queued or not reported"; exit 1; }
want=$(seq 3000 3039 | grep -vxE '3030|3002|3005|3007|3010' | tr '\n' ' ')
grep -q "^3007: REFUSED: conflicts with feat/parity.*owner sess-7 " $d/err || { echo "FAIL: the conflict line does not name the owner: $(cat $d/err)"; exit 1; }
grep -q "^3002: REFUSED: no .Owner" $d/err && grep -q "^3030: REFUSED: no .Owner" $d/err || { echo "FAIL: a PR without an Owner line was queued by default: $(cat $d/err)"; exit 1; }
off=$(REQUIRE_OWNER= PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>/dev/null | tr '\n' ' ')
want_off="3030 3002 $(seq 3000 3039 | grep -vxE '3030|3002|3005|3007|3010' | tr '\n' ' ')"
[ "$off" = "$want_off" ] || { echo "FAIL: REQUIRE_OWNER= does not switch the rule off: got '$off'"; exit 1; }
# a PR the last landing refused (same head, same feat/parity) is not queued again; a moved head or parity queues it
echo "3012 h3012 paritysha1" > $d/refused.state; echo "3013 h3013 oldparity" >> $d/refused.state
again=$(REFUSED_STATE=$d/refused.state PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>/dev/null | tr '\n' ' ')
echo "$again" | grep -qw 3012 && { echo "FAIL: a PR refused at this head and this feat/parity was queued again (the runner spun on it every second)"; exit 1; }
echo "$again" | grep -qw 3013 || { echo "FAIL: a PR refused against an older feat/parity was not queued again"; exit 1; }
[ "$out" = "$want" ] && echo PASS || { echo "FAIL"; echo " got: $out"; echo "want: $want"; exit 1; }
