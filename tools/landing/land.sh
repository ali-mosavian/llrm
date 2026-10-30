#!/usr/bin/env bash
# land.sh N... (checkout $LAND_WORKTREE, target $LAND_TARGET_DIR, lock dir $LAND_TMP; per-merger values run in parallel) : retarget, mark ready, then tools/parity_squash.py merge N (fast mode by default: merge onto the head, one workspace build check, squash with a head lease; LAND_FAST=0 for the full smoke).
export BUILDSLOT_BYPASS=1
export LAND_FAST=${LAND_FAST:-1}  # the user's call: no smoke, no tests; the build check stays
mkdir -p "${LAND_TMP:-/tmp/landing}"
source "$(dirname "$0")/refusal-lines.sh"
R=ali-mosavian/llrm; W=${LAND_WORKTREE:-~/scratch/coord-ps}; T=${LAND_TARGET_DIR:-~/scratch/coord-ps-target}
for n in "$@"; do
  info=$(gh pr view $n --repo $R --json state,isDraft,baseRefName,title -q '"\(.state)\t\(.isDraft)\t\(.baseRefName)\t\(.title)"') || { echo "$n: view failed"; continue; }
  IFS=$'\t' read -r st dr base title <<<"$info"
  [ "$st" = OPEN ] || { echo "$n: $st"; continue; }
  [ "$base" = feat/parity ] || { gh pr edit $n --repo $R --base feat/parity >/dev/null && echo "$n: retargeted from $base"; }
  [ "$dr" = true ] && { gh pr ready $n --repo $R >/dev/null && echo "$n: marked ready"; }
  ( flock -n 9 || { echo "waiting for the landing lock (another landing holds the checkout)" >&2; flock -x 9; }; cd $W && git fetch -q origin && git reset -q --hard && git checkout -q --detach origin/feat/parity \
    && WAITS=${WAITS:-$(cd "$(dirname "$0")" && pwd)/waits.txt} PARITY_BIN=$T/release CARGO_TARGET_DIR=$T CARGO_INCREMENTAL=0 uv run --project tools python tools/parity_squash.py merge-batch $n > ${LAND_TMP:-/tmp/landing}/sq-$n.log 2>&1 ) 9>"$W.checkout.lock"
  echo "$n: $(refusal_lines ${LAND_TMP:-/tmp/landing}/sq-$n.log)"
  if [ "$(gh pr view $n --repo $R --json state -q .state)" = MERGED ]; then echo "$n: MERGED  $title"
  else  # refused or failed: build-queue.sh skips this head against this feat/parity until one of them moves
    S=${REFUSED_STATE:-${LAND_TMP:-/tmp/landing}/refused.state}; touch "$S"; sed -i "/^$n /d" "$S"
    echo "$n $(gh pr view $n --repo $R --json headRefOid -q .headRefOid) $(gh api "repos/$R/git/ref/heads/feat/parity" -q .object.sha)" >> "$S"
  fi
done
