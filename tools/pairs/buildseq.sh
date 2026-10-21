#!/bin/bash
# buildseq.sh SHA...: dist binaries of each sha, one after another in the worktree $PAIR_WT (one target dir); products in $PAIR_SCRATCH/sbp-SHA9
cd "${PAIR_WT:?PAIR_WT: the worktree to build in}" || exit 1
S=${PAIR_SCRATCH:-$HOME/scratch}
for s in "$@"; do
  git checkout -q --detach $s && bash tools/pairs/pair_build.sh "$S" $(git rev-parse --short=9 HEAD) && echo "BUILT $s" || echo "FAILED $s"
done
echo ALLDONE
