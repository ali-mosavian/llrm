#!/bin/bash
# buildseq.sh SHA...: dist binaries of each sha, one after another, each in a worktree of its own made from $PAIR_WT's repository (one target dir); products in $PAIR_SCRATCH/sbp-SHA9
cd "${PAIR_WT:?PAIR_WT: the worktree to build in}" || exit 1
S=${PAIR_SCRATCH:-$HOME/scratch}
for s in "$@"; do
  bash tools/pairs/pair_build.sh "$S" $(git rev-parse --short=9 $s) && echo "BUILT $s" || echo "FAILED $s"
done
echo ALLDONE
