#!/usr/bin/env bash
# build-queue.sh: prints the landing order, one PR number per line, built from GitHub on every call:
# open, non-draft, base feat/parity, minus waits.txt (owner waits), the PRs of priority.txt first in its order, every other ready PR after, ascending.
# A hand-kept list dropped PRs; a ready PR missing from priority.txt still lands.
R=${R:-ali-mosavian/llrm}; HERE=$(cd "$(dirname "$0")" && pwd)
PRIORITY=${PRIORITY:-$HERE/priority.txt}; WAITS=${WAITS:-$HERE/waits.txt}
nums() { sed 's/#.*//' "$1" 2>/dev/null | tr -s ' ' '\n' | grep -E '^[0-9]+$'; }
ready=$(gh pr list --repo "$R" --state open --base feat/parity --limit 1000 --json number,isDraft -q '.[]|select(.isDraft==false)|.number') || exit 1
waiting=$(nums "$WAITS")
for n in $(nums "$PRIORITY") $(echo "$ready" | sort -n); do
  echo "$ready" | grep -qx "$n" && ! echo "$waiting" | grep -qx "$n" && echo "$n"
done | awk '!seen[$0]++'
