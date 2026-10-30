#!/bin/bash
# runpinned.sh SHA DIST OUT: tests/run by group, with the binaries in DIST (built at SHA), the tests and runtime from a tree checked out at SHA
# (PIN_TREES, default ~/scratch). Run it from any checkout of the repo; fails (exit 2) when the tree is not at SHA, and OUT ends with `done` only after run_groups ran.
set -u
SHA=$1; D=$2; O=$3
for t in jwasm jwlink dosbox-x; do [ -e "$D/$t" ] || { echo "runpinned: $D/$t is missing: the run would link and run with nothing" >&2; exit 2; }; done
REPO=$(git rev-parse --show-toplevel) || exit 2
T=${PIN_TREES:-$HOME/scratch}/pin-$SHA
[ -d "$T" ] || git -C "$REPO" worktree add -q --detach "$T" "$SHA" || exit 2
python3 "$REPO/tools/pairruns.py" --tree "$T" "$SHA" || exit 2
touch "$D"/llrm-* "$D/nibfront" 2>/dev/null
cd "$T" && LLRM_BIN=$D python3 tools/dosbatch/run_groups.py > "$O" 2>&1; echo done >> "$O"
