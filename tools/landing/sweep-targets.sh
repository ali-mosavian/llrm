#!/usr/bin/env bash
# sweep-targets.sh: runs prune-target.sh --apply on every cargo target dir over CAP_KB (default 15 GB) under SWEEP_ROOTS (default ~/scratch and the worktrees),
# active dirs included: it removes only stale hashes (see prune-target.sh), which the owner's next build recreates. Prints `dir: pruned N entries, X bytes` per dir.
HERE=$(cd "$(dirname "$0")" && pwd); cap=${CAP_KB:-15728640}
# default: ~/scratch, and every worktree of this repo (taken from git, so no tool-named directory is spelled out here)
roots=${SWEEP_ROOTS:-"$HOME/scratch $(git -C "$HERE" worktree list --porcelain | sed -n 's/^worktree //p' | xargs -r -n1 dirname | sort -u | tr '\n' ' ')"}
for root in $roots; do
  # every cargo target by its marker file, whatever the dir is called (tgt-*, */target, */*/target, *-target): a name glob missed ~/scratch/tgt-backend-two (56 GB)
  for d in $(find "$root" -maxdepth 4 \( -name .rustc_info.json -o -name CACHEDIR.TAG \) -printf '%h\n' 2>/dev/null | sort -u); do
    [ "$(du -sk "$d" 2>/dev/null | cut -f1)" -gt "$cap" ] || continue
    out=$("$HERE/prune-target.sh" "$d" --apply) && echo "$d: pruned $out"
  done
done
