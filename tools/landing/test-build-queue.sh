#!/usr/bin/env bash
# Regression: the hand-built queue dropped 17 of 58 ready PRs, and `gh pr list` without --limit silently returned 30. A stub gh returns only 30 PRs unless
# given --limit >= the count. The queue must hold every ready PR, minus waits, priority first, even those missing from priority.txt.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d)
export PREFLIGHT_REPO=$d/norepo  # no git repo: the pre-flight fails open and the cases below see GitHub's flag alone
cat > $d/gh <<'S'
#!/bin/sh
[ "$1" = api ] && { echo paritysha1; exit 0; }
limit=30; q=.; while [ $# -gt 0 ]; do case "$1" in --limit) limit=$2;; -q) q=$2;; esac; shift; done
i=0; while [ $i -lt 40 ]; do n=$((3000+i)); d=false; [ $i -eq 5 ] && d=true; m=MERGEABLE; [ $i -eq 7 ] && m=CONFLICTING; [ $i -eq 8 ] || [ $i -eq 9 ] && m=UNKNOWN; l=[]; [ $i -eq 11 ] && l='[{"name":"hold"}]'; b=""; case $i in 2|30) ;; *) b="Owner: sess-$i";; esac; echo "{\"number\":$n,\"isDraft\":$d,\"mergeable\":\"$m\",\"body\":\"$b\",\"headRefOid\":\"h$n\",\"labels\":$l}"; i=$((i+1)); done | head -n $limit | jq -s "$q" | jq -r 'if type=="array" then .[] else . end'
S
chmod +x $d/gh; echo "3030 3002" > $d/priority.txt; echo "3010 # waits" > $d/waits.txt
out=$(PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>$d/err | tr '\n' ' ')
grep -q "^3007: REFUSED: conflicts with release/next" $d/err || { echo "FAIL: a conflicting PR was queued or not reported"; exit 1; }
grep -q "^3011: REFUSED: label hold" $d/err || { echo "FAIL: a ready PR labelled hold was queued or not reported: $(cat $d/err)"; exit 1; }
want="3030 3002 $(seq 3000 3039 | grep -vxE '3030|3002|3005|3007|3010|3011' | tr '\n' ' ')"
grep -q "^3007: REFUSED: conflicts with release/next.*owner sess-7 " $d/err || { echo "FAIL: the conflict line does not name the owner: $(cat $d/err)"; exit 1; }
grep -q "^3002: WARNING: no .Owner" $d/err && grep -q "^3030: WARNING: no .Owner" $d/err || { echo "FAIL: a PR without an Owner line was not warned about by default: $(cat $d/err)"; exit 1; }
echo "$out" | grep -qw 3002 && echo "$out" | grep -qw 3030 || { echo "FAIL: a PR without an Owner line was not queued under the default warning: '$out'"; exit 1; }
refused=$(BODY_CHECKS=refuse PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>$d/err3 | tr '\n' ' ')
grep -q "^3002: REFUSED: no .Owner" $d/err3 && ! echo "$refused" | grep -qw 3002 || { echo "FAIL: BODY_CHECKS=refuse does not refuse a PR without an Owner line: $(cat $d/err3)"; exit 1; }
off=$(REQUIRE_OWNER= PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>/dev/null | tr '\n' ' ')
want_off="3030 3002 $(seq 3000 3039 | grep -vxE '3030|3002|3005|3007|3010|3011' | tr '\n' ' ')"
[ "$off" = "$want_off" ] || { echo "FAIL: REQUIRE_OWNER= does not switch the rule off: got '$off'"; exit 1; }
# a PR the last landing refused (same head, same release/next) is not queued again; a moved head or parity queues it
echo "3012 h3012 paritysha1" > $d/refused.state; echo "3013 h3013 oldparity" >> $d/refused.state
again=$(REFUSED_STATE=$d/refused.state PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>/dev/null | tr '\n' ' ')
echo "$again" | grep -qw 3012 && { echo "FAIL: a PR refused at this head and this release/next was queued again (the runner spun on it every second)"; exit 1; }
echo "$again" | grep -qw 3013 || { echo "FAIL: a PR refused against an older release/next was not queued again"; exit 1; }
# a PR refused for its own head stays out whatever release/next does, until its head changes (held.state)
printf '3014 h3014\n3015 hOTHER\n' > $d/held.state
held=$(HELD_STATE=$d/held.state REFUSED_STATE=$d/none.state PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>/dev/null | tr '\n' ' ')
echo "$held" | grep -qw 3014 && { echo "FAIL: a PR held at its current head was queued again"; exit 1; }
echo "$held" | grep -qw 3015 || { echo "FAIL: a PR held at an older head was not queued once its head changed"; exit 1; }
# a PR refused at an older head whose owner has just pushed (new head first seen now) lands alone for the grace; after it the normal order resumes
echo "3020 hOLD paritysha1" > $d/refused2.state
run() { NOW=$1 SEEN_STATE=$d/seen.state REFUSED_STATE=$d/refused2.state PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>$d/err2 | tr '\n' ' '; }
alone=$(run 1000)
[ "$alone" = "3020 " ] || { echo "FAIL: a freshly pushed PR did not land alone: got '$alone'"; exit 1; }
grep -q "^HOLD: landing 3020" $d/err2 || { echo "FAIL: the hold is not logged with the PR number"; exit 1; }
later=$(run 1700)
echo "$later" | grep -qw 3020 && echo "$later" | grep -qw 3021 || { echo "FAIL: after the grace the queue did not resume its normal order: got '$later'"; exit 1; }
# an unreadable release/next (a 403) keeps the old queue: it would match no refused.state line and re-queue every refused PR
mkdir -p $d/p403; printf '#!/bin/sh\n[ "$1" = api ] && exit 1\nexec %s/gh "$@"\n' "$d" > $d/p403/gh; chmod +x $d/p403/gh
PATH=$d/p403:$PATH "$here/build-queue.sh" >/dev/null 2>&1 && { echo "FAIL: build-queue.sh succeeded with release/next unreadable"; exit 1; }
# GitHub says UNKNOWN right after a landing: a PR that conflicts with release/next must not be queued then (#5269 was batched three times while CONFLICTING); one that merges is
printf '#!/bin/sh\n[ "$1" = 3008 ]\n' > $d/conflicts.sh; chmod +x $d/conflicts.sh
pre=$(PREFLIGHT_CMD=$d/conflicts.sh PATH=$d:$PATH PRIORITY=$d/priority.txt WAITS=$d/waits.txt "$here/build-queue.sh" 2>$d/err4 | tr '\n' ' ')
echo "$pre" | grep -qw 3008 && { echo "FAIL: an UNKNOWN PR that conflicts with release/next was queued"; exit 1; }
echo "$pre" | grep -qw 3009 || { echo "FAIL: an UNKNOWN PR that merges was dropped"; exit 1; }
grep -q "^3008: REFUSED: conflicts with release/next" $d/err4 || { echo "FAIL: the pre-flight conflict is not reported with its owner: $(cat $d/err4)"; exit 1; }
# the real check, on real git: FETCH_HEAD must be the PR's head, not release/next (fetching parity last compared parity with itself and called #5269 clean)
g() { git -C "$d/o" -c user.email=t@t -c user.name=t "$@"; }
git init -q --bare -b release/next $d/origin.git; git clone -q $d/origin.git $d/o 2>/dev/null; echo a > $d/o/f; g add f; g commit -qm base; g push -q origin HEAD:release/next
g checkout -q -b clash; echo b > $d/o/f; g commit -qam clash; g push -q origin HEAD:refs/pull/5001/head
g checkout -q release/next 2>/dev/null || g checkout -q -b main2 origin/release/next; g checkout -q -b clean origin/release/next; echo c > $d/o/g; g add g; g commit -qm clean; g push -q origin HEAD:refs/pull/5002/head
g checkout -q origin/release/next 2>/dev/null; echo parity > $d/o/f; g commit -qam move; g push -q origin HEAD:release/next
mkdir -p $d/real; cat > $d/real/gh <<'S'
#!/bin/sh
[ "$1" = api ] && { echo paritysha1; exit 0; }
echo "5001 UNKNOWN a h1"; echo "5002 UNKNOWN b h2"
S
chmod +x $d/real/gh
# Regression: each of the pre-flight's 160 fetches ran `git maintenance run --auto` (reflog expire in state D): build-queue took 10 min at 23:47 and the pass started 25 min late.
realgit=$(command -v git); cat > $d/real/git <<S
#!/bin/sh
echo "\$*" >> $d/gitlog
exec $realgit "\$@"
S
chmod +x $d/real/git
realq=$(PREFLIGHT_REPO=$d/o PATH=$d/real:$PATH PRIORITY=/dev/null WAITS=/dev/null "$here/build-queue.sh" 2>$d/err5 | tr '\n' ' ')
grep " fetch " $d/gitlog | grep -qv "maintenance.auto=false" && { echo "FAIL: a pre-flight fetch ran auto maintenance: $(grep " fetch " $d/gitlog | grep -v maintenance.auto=false | head -2)"; exit 1; }
echo "$realq" | grep -qw 5001 && { echo "FAIL: a PR that conflicts on real git was queued: $(cat $d/err5)"; exit 1; }
echo "$realq" | grep -qw 5002 || { echo "FAIL: a PR that merges cleanly on real git was dropped: $(cat $d/err5)"; exit 1; }
[ "$out" = "$want" ] && echo PASS || { echo "FAIL"; echo " got: $out"; echo "want: $want"; exit 1; }
