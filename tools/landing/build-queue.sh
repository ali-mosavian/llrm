#!/usr/bin/env bash
# build-queue.sh: prints the landing order, one PR number per line, built from GitHub on every call:
# open, non-draft, base feat/parity, minus waits.txt (owner waits), the PRs of priority.txt first in its order, every other ready PR after, ascending.
# A hand-kept list dropped PRs; a ready PR missing from priority.txt still lands.
R=${R:-ali-mosavian/llrm}; HERE=$(cd "$(dirname "$0")" && pwd)
PRIORITY=${PRIORITY:-$HERE/priority.txt}; WAITS=${WAITS:-$HERE/waits.txt}
nums() { sed 's/#.*//' "$1" 2>/dev/null | tr -s ' ' '\n' | grep -E '^[0-9]+$'; }
ready=$(gh pr list --repo "$R" --state open --base feat/parity --limit 1000 --json number,isDraft,mergeable,body -q '.[]|select(.isDraft==false)|"\(.number) \(.mergeable) \((.body // "")|capture("(^|\n)Owner: *(?<o>[A-Za-z0-9_.-]+)")?.o // "-")"') || exit 1
# GitHub's mergeable flag is against the base: a PR that conflicts with feat/parity cannot land, alone or in a batch
echo "$ready" | awk '$2=="CONFLICTING"{printf "%s: REFUSED: conflicts with feat/parity (GitHub mergeable=CONFLICTING); owner %s merges fresh feat/parity\n", $1, $3}' >&2
# Owner: <session> in the PR body names who to tell; a PR without it is refused (REQUIRE_OWNER= switches the rule off)
REQUIRE_OWNER=${REQUIRE_OWNER-1}
[ -n "$REQUIRE_OWNER" ] && echo "$ready" | awk '$2!="CONFLICTING" && $3=="-"{printf "%s: REFUSED: no `Owner: <session>` line in the description\n", $1}' >&2
ready=$(echo "$ready" | awk -v req="$REQUIRE_OWNER" '$2!="CONFLICTING" && !(req!="" && $3=="-"){print $1}')
waiting=$(nums "$WAITS")
for n in $(nums "$PRIORITY") $(echo "$ready" | sort -n); do
  echo "$ready" | grep -qx "$n" && ! echo "$waiting" | grep -qx "$n" && echo "$n"
done | awk '!seen[$0]++'
