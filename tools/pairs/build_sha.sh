#!/bin/bash
# build_sha.sh REPO SCRATCH SHA: the dist binaries of commit SHA as SCRATCH/sbp-SHA/dist, built in a git worktree of their own
# (SCRATCH/wt-pair-SHA, reused when present) and never in a directory two builds share: a shared worktree that another session
# checked out during a 12-minute build gave a number for the wrong commit. The worktree's HEAD before and the binary's stamp after
# are tripwires, not the protection.
set -eu
R=$1; S=$2; SHA=$3; T=$S/sbp-$SHA; HERE=$(cd "$(dirname "$0")" && pwd)
[ -x "$T/dist/llrm-c" ] && [ -e "$T/ok" ] && exit 0
FULL=$(git -C "$R" rev-parse --verify "$SHA^{commit}")
WT=$S/wt-pair-$SHA
(
  flock 9
  [ -e "$WT/.git" ] || flock "$S/wt-pair-add.lock" git -C "$R" worktree add -q --detach "$WT" "$FULL"
  [ "$(git -C "$WT" rev-parse HEAD)" = "$FULL" ] || { echo "build_sha: $WT is not at $FULL" >&2; exit 3; }
  (cd "$WT" && bash "$HERE/pair_build.sh" "$S" "$SHA")
  [ "$(git -C "$WT" rev-parse HEAD)" = "$FULL" ] || { echo "build_sha: $WT moved during the build of $FULL" >&2; exit 3; }
  # One target per sha is 7 GB: only the binaries are kept (PAIR_KEEP_TARGET=1 keeps it for an incremental rebuild).
  [ -n "${PAIR_KEEP_TARGET:-}" ] || rm -rf "$S/tgt-dist-wt-pair-$SHA"
) 9>"$S/wt-pair-$SHA.lock"
python3 "$HERE/../stamp.py" --expect-sha "$FULL" "$T/dist/llrm-c"
