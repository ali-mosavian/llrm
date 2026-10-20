#!/bin/bash
# pair_build.sh SCRATCH SHA: the dist binaries of the checkout in $PWD, as SCRATCH/sbp-SHA/dist (llrm-c, llrm-qb, llrm-nib, nibfront, their .d, and jwasm/jwlink/dosbox-x when the build made them: run_tests links and runs with them), built in ONE
# persistent dist target per worktree (SCRATCH/tgt-dist-<worktree>) under a lock. A target dir per sha left a whole build (0.7 GB of binaries, 7+ GB of deps) for
# each of 76 pairs: 51 GB in a day on a host that fell 50 GB/h toward its alarm.
set -e
S=$1; SHA=$2; T=$S/sbp-$SHA; W=$(basename "$PWD")
[ -x "$T/dist/llrm-c" ] && [ -e "$T/ok" ] && exit 0
flock "$S/tgt-dist-$W.lock" bash -c '
  S=$0; T=$1; B=$S/tgt-dist-$2
  nice -n 10 env -u RUSTFLAGS CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=$B ${PAIR_CARGO:-buildslot cargo} build --profile dist -q --bin llrm-c --bin llrm-qb --bin llrm-nib --bin nibfront
  mkdir -p $T/dist && cp -p $B/dist/llrm-c $B/dist/llrm-qb $B/dist/llrm-nib $B/dist/nibfront $B/dist/*.d $T/dist/ && for t in jwasm jwlink dosbox-x; do [ -e $B/dist/$t ] && cp -p $B/dist/$t $T/dist/; done && touch $T/ok
' "$S" "$T" "$W"
