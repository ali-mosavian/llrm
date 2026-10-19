#!/usr/bin/env bash
# unlogged.sh [GIT_RANGE]: prints every commit of feat/parity's first-parent history (default: the last 6 hours) whose sha is not in landed.log, with its PR,
# the PR's `Owner:` line and its time: a landing that did not go through the gate. Exit 1 when there is any.
LANDED_LOG=${LANDED_LOG:-$HOME/scratch/coordwork/landed.log}; R=${R:-ali-mosavian/llrm}
range=${1:---since=6.hours.ago origin/feat/parity}
git log --first-parent --format='%H %ct %s' $range | while read -r sha ts subject; do
  grep -q "^$sha " "$LANDED_LOG" && continue
  pr=$(gh api "repos/$R/commits/$sha/pulls" -q '.[0].number' 2>/dev/null)
  owner=$([ -n "$pr" ] && gh pr view "$pr" --repo "$R" --json body -q .body 2>/dev/null | sed -n 's/^Owner: *//p' | head -1)
  echo "$(date -d "@$ts" +%H:%M) ${sha:0:9} #${pr:-?} owner ${owner:--}: $subject"
done | { out=$(cat); [ -n "$out" ] && { echo "$out"; exit 1; }; exit 0; }
