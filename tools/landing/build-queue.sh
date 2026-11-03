#!/usr/bin/env bash
# build-queue.sh: prints the landing order, one PR number per line, built from GitHub on every call:
# open, non-draft, base $LAND_BRANCH, minus waits.txt (owner waits), the PRs of priority.txt first in its order, every other ready PR after, ascending.
# A hand-kept list dropped PRs; a ready PR missing from priority.txt still lands.
. "$(dirname "${BASH_SOURCE[0]:-$0}")/branch.sh"
R=${R:-ali-mosavian/llrm}; HERE=$(cd "$(dirname "$0")" && pwd)
PRIORITY=${PRIORITY:-$HERE/priority.txt}; WAITS=${WAITS:-$HERE/waits.txt}
nums() { sed 's/#.*//' "$1" 2>/dev/null | tr -s ' ' '\n' | grep -E '^[0-9]+$'; }
ready=$(gh pr list --repo "$R" --state open --base $LAND_BRANCH --limit 1000 --json number,isDraft,mergeable,body,headRefOid,labels -q '.[]|select(.isDraft==false)|"\(.number) \(if any(.labels[]?; .name=="hold") then "HOLD" else .mergeable end) \((.body // "")|capture("(^|\n)Owner: *(?<o>[A-Za-z0-9_.-]+)")?.o // "-") \(.headRefOid // "-")"') || exit 1
# a PR labelled `hold` is never queued, ready or not, priority.txt or not (its mergeable column carries HOLD from the query)
echo "$ready" | awk '$2=="HOLD"{printf "%s: REFUSED: label hold\n", $1}' >&2
ready=$(echo "$ready" | awk '$2!="HOLD"')
# a PR the last landing refused is not tried again until its head or $LAND_BRANCH moves (refused.state, written by land.sh): the runner re-queued it every second
STATE=${REFUSED_STATE:-${LAND_TMP:-/tmp/landing}/refused.state}
# an If-None-Match request (pr_state.py parity): an unchanged ref answers 304, which costs no quota
parity=$(python3 "$HERE/pr_state.py" parity 2>/dev/null); [ -n "$parity" ] || parity=$(gh api "repos/$R/git/ref/heads/$LAND_BRANCH" -q .object.sha 2>/dev/null)
# an unreadable parity (a 403) would match no refused.state line and re-queue every refused PR: keep the old queue
[ -n "$parity" ] || { echo "gh could not read $LAND_BRANCH (rate limit?); keeping the old queue" >&2; exit 1; }
ready=$(echo "$ready" | while read -r n m o h; do grep -qx "$n $h $parity" "$STATE" 2>/dev/null || echo "$n $m $o $h"; done)
# a PR refused for its own head stays out until that head changes (held.state, written by the runner: `<PR> <head sha>`); a retry at the same head cost a batch each time
HELD=${HELD_STATE:-${LAND_TMP:-/tmp/landing}/held.state}
ready=$(echo "$ready" | while read -r n m o h; do grep -qx "$n $h" "$HELD" 2>/dev/null || echo "$n $m $o $h"; done)
# GitHub reports UNKNOWN for every PR for a while after each landing, and only the literal CONFLICTING is dropped below: #5269 (CONFLICTING since 07:25) was batched at 13:41, 14:41 and 15:08.
# A PR that is not MERGEABLE is merged against $LAND_BRANCH locally (git merge-tree, no checkout); a conflict marks it CONFLICTING. A failed fetch keeps it (fail open).
# PREFLIGHT_CMD overrides the check in tests: `$PREFLIGHT_CMD <PR>` exits 0 when the PR conflicts.
REPO=${PREFLIGHT_REPO:-$(cd "$HERE/../.." && pwd)}
# -c maintenance.auto=false -c gc.auto=0: every fetch ran `git maintenance run --auto` (reflog expire, minutes under load): ~80 PRs x 2 fetches took build-queue 10 min at 23:47.
conflicts_with_parity() {
  [ -n "$PREFLIGHT_CMD" ] && { $PREFLIGHT_CMD "$1"; return; }
  git -C "$REPO" -c maintenance.auto=false -c gc.auto=0 fetch -q origin $LAND_BRANCH 2>/dev/null && git -C "$REPO" -c maintenance.auto=false -c gc.auto=0 fetch -q origin "refs/pull/$1/head" 2>/dev/null && ! git -C "$REPO" merge-tree --write-tree origin/$LAND_BRANCH FETCH_HEAD >/dev/null 2>&1
}
ready=$(echo "$ready" | while read -r n m o h; do [ "$m" != MERGEABLE ] && [ "$m" != CONFLICTING ] && conflicts_with_parity "$n" && m=CONFLICTING; echo "$n $m $o $h"; done)
# GitHub's mergeable flag is against the base: a PR that conflicts with $LAND_BRANCH cannot land, alone or in a batch
echo "$ready" | awk -v b="$LAND_BRANCH" '$2=="CONFLICTING"{printf "%s: REFUSED: conflicts with %s (GitHub mergeable=CONFLICTING); owner %s merges fresh %s (push rejected? git pull --no-rebase, then push; never force)\n", $1, b, $3, b}' >&2
# Owner: <session> in the PR body names who to tell. BODY_CHECKS=warn (the default, the user's call while the backend parts land) only warns in the log and the PR
# is queued; BODY_CHECKS=refuse refuses it. REQUIRE_OWNER= switches the check off.
BODY_CHECKS=${BODY_CHECKS:-warn}
REQUIRE_OWNER=${REQUIRE_OWNER-1}
label=WARNING; refuse_owner=""; [ "$BODY_CHECKS" = refuse ] && { label=REFUSED; refuse_owner=$REQUIRE_OWNER; }
[ -n "$REQUIRE_OWNER" ] && echo "$ready" | awk -v l="$label" '$2!="CONFLICTING" && $3=="-"{printf "%s: %s: no `Owner: <session>` line in the description\n", $1, l}' >&2
heads="$ready"
ready=$(echo "$ready" | awk -v req="$refuse_owner" '$2!="CONFLICTING" && !(req!="" && $3=="-"){print $1}')
waiting=$(nums "$WAITS")
queue=$(for n in $(nums "$PRIORITY") $(echo "$ready" | sort -n); do
  echo "$ready" | grep -qx "$n" && ! echo "$waiting" | grep -qx "$n" && echo "$n"
done | awk '!seen[$0]++')
# serialize a fresh push: a PR refused before whose head has moved since (its owner just merged $LAND_BRANCH and pushed) lands next, alone, for GRACE seconds
# from the moment its new head is first seen (default 10 min, then the normal order resumes: a dead owner cannot block the queue). Other landings in between
# are what made the head stale again, five times for #3344.
SEEN=${SEEN_STATE:-${LAND_TMP:-/tmp/landing}/head-seen.state}; GRACE=${GRACE:-600}; NOW=${NOW:-$(date +%s)}; touch "$SEEN"
fresh=""
for n in $queue; do
  cur=$(echo "$heads" | awk -v n="$n" '$1==n{print $4}'); old=$(awk -v n="$n" '$1==n{print $2}' "$STATE" 2>/dev/null | tail -1)
  [ -n "$old" ] && [ "$old" != "$cur" ] || continue
  first=$(awk -v n="$n" -v h="$cur" '$1==n && $2==h{print $3}' "$SEEN" | tail -1)
  [ -n "$first" ] || { first=$NOW; echo "$n $cur $NOW" >> "$SEEN"; }
  [ $((NOW - first)) -lt "$GRACE" ] && fresh="$fresh $n"
done
if [ -n "$fresh" ]; then
  echo "HOLD: landing${fresh} alone for up to ${GRACE}s from their new head (others wait one cycle): $(echo "$queue" | tr '\n' ' ')" >&2
  for n in $fresh; do echo "$n"; done
else echo "$queue"; fi
