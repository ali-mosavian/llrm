#!/usr/bin/env bash
# sweep-targets.sh: runs prune-target.sh --apply on every cargo target dir over CAP_KB (default 15 GB) under SWEEP_ROOTS (default ~/scratch and the worktrees),
# active dirs included: it removes only stale hashes (see prune-target.sh), which the owner's next build recreates. Prints `dir: pruned N entries, X bytes` per dir.
HERE=$(cd "$(dirname "$0")" && pwd); cap=${CAP_KB:-15728640}
# default: ~/scratch, and every worktree of this repo (taken from git, so no tool-named directory is spelled out here)
roots=${SWEEP_ROOTS:-"$HOME/scratch $(git -C "$HERE" worktree list --porcelain | sed -n 's/^worktree //p' | xargs -r -n1 dirname | sort -u | tr '\n' ' ')"}
for root in $roots; do
  # every cargo target by its marker file, whatever the dir is called (tgt-*, */target, */*/target, *-target): a name glob missed ~/scratch/tgt-backend-two (56 GB)
  for d in $(find "$root" -maxdepth 4 \( -name .rustc_info.json -o -name CACHEDIR.TAG \) -printf '%h\n' 2>/dev/null | sort -u); do
    size=$(du -sk "$d" 2>/dev/null | cut -f1)
    if [ "$size" -le "$cap" ]; then
      # a target under the cap that nothing has touched for IDLE_HOURS (default 3) and that holds over IDLE_CAP_KB (default 1 GB) is pruned too: the six alone slots of ~/.cache/llrm
      # were 18 dirs of 7-15 GB each, all under the cap, 139 GB idle for 6 h
      newest=$(find "$d" -printf '%T@\n' 2>/dev/null | sort -n | tail -1 | cut -d. -f1)
      [ "$size" -gt "${IDLE_CAP_KB:-1048576}" ] && [ -n "$newest" ] && [ $(( $(date +%s) - newest )) -gt $(( ${IDLE_HOURS:-3} * 3600 )) ] || continue
      # a process with a file open under it (a running build or test) keeps it
      [ -z "$(lsof -t +D "$d" 2>/dev/null | head -1)" ] || continue
    fi
    [ -n "${DRY:-}" ] && { echo "would prune stale hashes of: $d"; continue; }
    out=$("$HERE/prune-target.sh" "$d" --apply) && echo "$d: pruned $out"
  done
done

# pair_build dists (`<root>/<name>/{ok,dist/llrm-c}`, 250 MB each, 400 of them by 10-24) and their `wt-pair-*` checkouts: pruned when older than DIST_MIN (default 180) minutes and no
# process has the directory as its cwd or in its command line. `ok` is pair_build's marker; a dist without it is its owner's and is only listed (`kept: ...`). DRY=1 lists and removes nothing.
# One snapshot of every process's cwd and command line, taken once: a scan of /proc per candidate took longer than the sweep's cadence.
procs=$(mktemp); trap 'rm -f "$procs"' EXIT
{ for p in /proc/[0-9]*; do readlink "$p/cwd"; tr '\0' ' ' < "$p/cmdline"; echo; done; } > "$procs" 2>/dev/null
in_use() { grep -qF -- "$1" "$procs"; }
tip=${TIP_SHA:-$(git -C "$HERE" rev-parse --verify -q origin/feat/parity 2>/dev/null)}
for root in $roots; do
  for holder in $(find "$root" -maxdepth 3 -path '*/dist/llrm-c' -mmin +"${DIST_MIN:-180}" -printf '%h\n' 2>/dev/null | xargs -r -n1 dirname | sort -u); do
    [ -e "$holder/ok" ] || { echo "kept (no ok marker): $holder"; continue; }
    name=$(basename "$holder")
    # Never the pinned base (every base-vs-new measure compares against it) nor the dist of the tip last landed (the next pair's base): TIP_SHA overrides the head read from origin/feat/parity.
    case "$name" in sbp-${DIST_PINNED:-854bb7efc}*) echo "kept (pinned base): $holder"; continue;; esac
    [ -n "$tip" ] && case "$name" in sbp-"$tip"*|sbp-"${tip:0:9}"*) echo "kept (last landed tip): $holder"; continue;; esac
    in_use "$holder" && { echo "kept (in use): $holder"; continue; }
    pair="$(dirname "$holder")/wt-pair-${name#sbp-}"
    kb=$(du -sk "$holder" 2>/dev/null | cut -f1)
    if [ -n "${DRY:-}" ]; then echo "would remove: $holder ${kb} KB"; continue; fi
    rm -rf -- "$holder" && echo "dist: removed $holder ${kb} KB"
    case "$name" in sbp-*) [ -d "$pair" ] && ! in_use "$pair" && rm -rf -- "$pair" && echo "dist: removed $pair";; esac
  done
done

# Orphan `wt-pair-X` checkouts: the loop above removes one only in the pass that removes its dist, so a checkout whose sbp-X went another way (a pruned `-n` rebuild, a
# failed build, an older sweep) stood for good: 72 of 90, ~89 GB. One older than DIST_MIN with no sbp-X of its sha, outside the pinned base and the last tip, that no process
# has as its cwd or in its command line, goes (pair_build makes a fresh checkout under tgt-dist.lock, so a build in flight is minutes old; the lock itself is held nearly always). Only `wt-pair-*` in a root itself.
for root in $roots; do
  [ -d "$root" ] || continue
  for pair in $(find "$root" -maxdepth 1 -name 'wt-pair-*' -type d -mmin +"${DIST_MIN:-180}" 2>/dev/null); do
    sha=${pair##*/wt-pair-}
    case "$sha" in "${DIST_PINNED:-854bb7efc}"*) continue;; esac
    [ -n "$tip" ] && case "$sha" in "$tip"*|"${tip:0:9}"*) continue;; esac
    ls -d "$root/sbp-$sha" "$root/sbp-${sha:0:9}" "$root/sbp-${sha:0:10}" "$root/sbp-$sha-"* 2>/dev/null | grep -q . && continue
    in_use "$pair" && { echo "kept (in use): $pair"; continue; }
    kb=$(du -sk "$pair" 2>/dev/null | cut -f1)
    if [ -n "${DRY:-}" ]; then echo "would remove orphan: $pair ${kb} KB"; continue; fi
    git -C "$HERE" worktree remove --force "$pair" 2>/dev/null || rm -rf -- "$pair"
    [ -e "$pair" ] && rm -rf -- "$pair"
    echo "orphan: removed $pair ${kb} KB"
  done
done
