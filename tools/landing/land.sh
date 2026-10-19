#!/usr/bin/env bash
# land.sh N... : retarget, mark ready, then tools/parity_squash.py merge N (lock, merged-tree build + smoke, subset rule, head-sha lease).
export BUILDSLOT_BYPASS=1
R=ali-mosavian/llrm; W=~/scratch/coord-ps; T=~/scratch/coord-ps-target
for n in "$@"; do
  info=$(gh pr view $n --repo $R --json state,isDraft,baseRefName,title -q '"\(.state)\t\(.isDraft)\t\(.baseRefName)\t\(.title)"') || { echo "$n: view failed"; continue; }
  IFS=$'\t' read -r st dr base title <<<"$info"
  [ "$st" = OPEN ] || { echo "$n: $st"; continue; }
  [ "$base" = feat/parity ] || { gh pr edit $n --repo $R --base feat/parity >/dev/null && echo "$n: retargeted from $base"; }
  [ "$dr" = true ] && { gh pr ready $n --repo $R >/dev/null && echo "$n: marked ready"; }
  ( cd $W && git fetch -q origin && git reset -q --hard && git checkout -q --detach origin/feat/parity \
    && PARITY_BIN=$T/release CARGO_TARGET_DIR=$T CARGO_INCREMENTAL=0 uv run --project tools python tools/parity_squash.py merge $n $KNOWN_RED > /tmp/claude-1000/sq-$n.log 2>&1 )
  echo "$n: $(grep -o 'REFUSED.*' /tmp/claude-1000/sq-$n.log | head -2 | tr '\n' ' ' | cut -c1-500)"
  [ "$(gh pr view $n --repo $R --json state -q .state)" = MERGED ] && echo "$n: MERGED  $title"
done
