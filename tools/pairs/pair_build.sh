#!/bin/bash
# pair_build.sh SCRATCH SHA: the dist binaries (llrm-c, llrm-qb, llrm-nib, nibfront, their .d, and jwasm/jwlink/dosbox-x when the build made them: run_tests links and runs with them) of commit SHA, as SCRATCH/sbp-SHA/dist.
# Run in any checkout of the repository. The commit is built in a worktree of its own (SCRATCH/wt-pair-<full sha>, made for the build and removed after it), never by `git checkout` in a
# shared directory: another session moving wt-head10 mid-build left a binary of an unknown tree under a sha's name (the 65eed2ad7 dist, and numbers taken from it). HEAD is checked before
# and after the build as a tripwire. One persistent dist target (SCRATCH/tgt-dist) under a lock holds the build of every sha in turn: a target dir per sha left a whole build (0.7 GB of binaries,
# 7+ GB of deps) for each of 76 pairs, 51 GB in a day.
set -e
S=$1; SHA=$2; T=$S/sbp-$SHA
[ -x "$T/dist/llrm-c" ] && [ -e "$T/ok" ] && exit 0
FULL=$(git rev-parse --verify -q "$SHA^{commit}") || { echo "pair_build: $SHA is not a commit of $PWD" >&2; exit 1; }
REPO=$(git rev-parse --show-toplevel)
flock "$S/tgt-dist.lock" bash -c '
  S=$0; T=$1; FULL=$2; REPO=$3; B=$S/tgt-dist; W=$S/wt-pair-$FULL
  git -C "$REPO" worktree remove --force "$W" 2>/dev/null || true
  git -C "$REPO" worktree add -q --detach "$W" "$FULL"
  trap "git -C \"$REPO\" worktree remove --force \"$W\" 2>/dev/null || true" EXIT
  at() { [ "$(git -C "$W" rev-parse HEAD)" = "$FULL" ] || { echo "pair_build: $W is at $(git -C "$W" rev-parse --short HEAD), not $FULL $1" >&2; exit 1; }; }
  at "before the build"
  (cd "$W" && nice -n 10 env -u RUSTFLAGS CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=$B ${PAIR_CARGO:-buildslot cargo} build --profile dist -q --bin llrm-c --bin llrm-qb --bin llrm-nib --bin nibfront)
  at "after the build"
  mkdir -p $T/dist && cp -p $B/dist/llrm-c $B/dist/llrm-qb $B/dist/llrm-nib $B/dist/nibfront $B/dist/*.d $T/dist/ && for t in jwasm jwlink dosbox-x; do [ -e $B/dist/$t ] && cp -p $B/dist/$t $T/dist/; done && touch $T/ok
' "$S" "$T" "$FULL" "$REPO"
