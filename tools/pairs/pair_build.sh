#!/bin/bash
# pair_build.sh SCRATCH SHA: the dist binaries (llrm-c, llrm-qb, llrm-nib, nibfront, their .d, and jwasm/jwlink/dosbox-x when the build made them: run_tests links and runs with them) of commit SHA, as SCRATCH/sbp-SHA/dist.
# Run in any checkout of the repository. The commit is built in a worktree of its own (SCRATCH/wt-pair-<full sha>, pruned by age), never by `git checkout` in a
# shared directory: another session moving wt-head10 mid-build left a binary of an unknown tree under a sha's name (the 65eed2ad7 dist, and numbers taken from it). HEAD is checked before
# and after the build as a tripwire. One persistent dist target (SCRATCH/tgt-dist) under a lock holds the build of every sha in turn: a target dir per sha left a whole build (0.7 GB of binaries,
# 7+ GB of deps) for each of 76 pairs, 51 GB in a day.
set -e
export PAIR_HERE=$(cd "$(dirname "$0")" && pwd)  # dist-bins.sh comes from this script, not from the commit being built (old ones lack it)
S=$1; SHA=$2; T=$S/sbp-$SHA
# `ok` is write-once: the md5 of every binary and the build line, the files read-only. A dist already built is checked against it and kept, never overwritten (another session
# copied a different llrm-c over sbp-65eed2ad7b…/dist after its build); PAIR_REBUILD=1 builds it again into sbp-SHA-<n>. Readers check the md5 (tools/stamp.py).
verify() { [ -s "$1/ok" ] && grep -qE '^[0-9a-f]{32}  ' "$1/ok" || return 0; (cd "$1/dist" && grep -E '^[0-9a-f]{32}  ' ../ok | md5sum -c --quiet - >/dev/null 2>&1) || { echo "pair_build: $1/dist is not what its ok recorded: refused" >&2; return 1; }; }
if [ -x "$T/dist/llrm-c" ] && [ -e "$T/ok" ]; then
  if [ -z "${PAIR_REBUILD:-}" ]; then verify "$T"; exit $?; fi
  n=2; while [ -e "$S/sbp-$SHA-$n" ]; do n=$((n + 1)); done; T=$S/sbp-$SHA-$n
fi
FULL=$(git rev-parse --verify -q "$SHA^{commit}") || { echo "pair_build: $SHA is not a commit of $PWD" >&2; exit 1; }
REPO=$(git rev-parse --show-toplevel)
flock "$S/tgt-dist.lock" bash -c '
  set -e  # the outer set -e does not reach this shell: a failed build went on to copy the previous build under this sha
  S=$0; T=$1; FULL=$2; REPO=$3; B=$S/tgt-dist; W=$S/wt-pair-$FULL
  # An old tree goes only when no dist of its sha is left (llrm-qb and llrm-nib may still read it).
  for old in $(find "$S" -maxdepth 1 -name "wt-pair-*" -mmin +${PAIR_KEEP_MIN:-4320}); do
    sha=${old##*/wt-pair-}
    ls -d "$S/sbp-$sha" "$S/sbp-${sha:0:9}" "$S/sbp-${sha:0:10}" "$S/sbp-$sha-"* 2>/dev/null | grep -q . && continue
    git -C "$REPO" worktree remove --force "$old" 2>/dev/null || rm -rf "$old"
  done
  git -C "$REPO" worktree remove --force "$W" 2>/dev/null || true
  git -C "$REPO" worktree add -q --detach "$W" "$FULL"
  at() { [ "$(git -C "$W" rev-parse HEAD)" = "$FULL" ] || { echo "pair_build: $W is at $(git -C "$W" rev-parse --short HEAD), not $FULL $1" >&2; exit 1; }; }
  at "before the build"
  . "$PAIR_HERE/dist-bins.sh"; EXTRA=$(dist_extra_bins "$W"); EXTRAFLAGS=; for b in $EXTRA; do EXTRAFLAGS="$EXTRAFLAGS --bin $b"; done; FEATURES=$(dist_features "$W"); [ -n "$FEATURES" ] && EXTRAFLAGS="$EXTRAFLAGS --features $FEATURES"
  (cd "$W" && nice -n 10 env SOURCE_DATE_EPOCH=0 RUSTFLAGS="--remap-path-prefix=$W=/llrm --remap-path-prefix=$B=/target --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/cargo" CARGO_INCREMENTAL=0 BUILDSLOT_PRIORITY=0 CARGO_TARGET_DIR=$B ${PAIR_CARGO:-buildslot cargo} build --profile dist -q --bin llrm-c --bin llrm-qb --bin llrm-nib --bin nibfront $EXTRAFLAGS)
  at "after the build"
  # a build that left the previous one in place (cargo found nothing to do, or failed quietly) is copied under this sha otherwise: 4 dists (5e4cac094d, aa8170fbbd, 8cb8ed34df, 3d22dc1faa) held another commit'"'"'s binary
  grep -q "wt-pair-$FULL/" $B/dist/llrm-c.d 2>/dev/null || { echo "pair_build: $B/dist/llrm-c.d does not name $W: the binary was not built from $FULL" >&2; exit 1; }
  mkdir -p $T/dist && cp -p $B/dist/llrm-c $B/dist/llrm-qb $B/dist/llrm-nib $B/dist/nibfront $B/dist/*.d $T/dist/ && for t in $EXTRA jwasm jwlink dosbox-x; do [ -e $B/dist/$t ] && cp -p $B/dist/$t $T/dist/; done
  { echo "sha: $FULL"; echo "cmd: build --profile dist --bin llrm-c --bin llrm-qb --bin llrm-nib --bin nibfront$EXTRAFLAGS (${PAIR_CARGO:-buildslot cargo}, CARGO_TARGET_DIR=$B)"; (cd $T/dist && md5sum *); } > $T/ok.part
  mv $T/ok.part $T/ok && chmod a-w $T/dist/* $T/ok
' "$S" "$T" "$FULL" "$REPO"
