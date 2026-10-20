#!/usr/bin/env bash
# land.sh N... : retarget, mark ready, then tools/parity_squash.py merge N (fast mode by default: merge onto the head, one workspace build check, squash with a head lease; LAND_FAST=0 for the full smoke).
export BUILDSLOT_BYPASS=1
export LAND_FAST=${LAND_FAST:-1}  # the user's call: no smoke, no tests; the build check stays
mkdir -p "${LAND_TMP:-/tmp/landing}"
R=ali-mosavian/llrm; W=~/scratch/coord-ps; T=~/scratch/coord-ps-target
for n in "$@"; do
  info=$(gh pr view $n --repo $R --json state,isDraft,baseRefName,title -q '"\(.state)\t\(.isDraft)\t\(.baseRefName)\t\(.title)"') || { echo "$n: view failed"; continue; }
  IFS=$'\t' read -r st dr base title <<<"$info"
  [ "$st" = OPEN ] || { echo "$n: $st"; continue; }
  [ "$base" = feat/parity ] || { gh pr edit $n --repo $R --base feat/parity >/dev/null && echo "$n: retargeted from $base"; }
  [ "$dr" = true ] && { gh pr ready $n --repo $R >/dev/null && echo "$n: marked ready"; }
  ( flock -x 9; cd $W && git fetch -q origin && git reset -q --hard && git checkout -q --detach origin/feat/parity \
    && PARITY_BIN=$T/release CARGO_TARGET_DIR=$T CARGO_INCREMENTAL=0 uv run --project tools python tools/parity_squash.py merge-batch $n > ${LAND_TMP:-/tmp/landing}/sq-$n.log 2>&1 ) 9>"${LAND_TMP:-/tmp/landing}/checkout.lock"
  echo "$n: $(grep -o 'REFUSED.*' ${LAND_TMP:-/tmp/landing}/sq-$n.log | head -2 | tr '\n' ' ' | cut -c1-500)"
  [ "$(gh pr view $n --repo $R --json state -q .state)" = MERGED ] && echo "$n: MERGED  $title"
done
