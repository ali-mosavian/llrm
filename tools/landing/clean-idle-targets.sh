#!/usr/bin/env bash
# clean-idle-targets.sh [--hours N] [--apply]: the cargo targets under SWEEP_ROOTS (default ~/scratch and the repo's worktrees) idle for more than N hours (default 72),
# found by their marker file. Dry run unless --apply. --apply removes only the cargo profile dirs (debug, release, dist, fast, incremental, tmp and any other dir holding
# a .fingerprint); logs, bench data and gate binaries stay. Per dir, just before it is touched: idle again (newest mtime of the dir and its profile dirs), and not in
# use: no process with its cwd, CARGO_TARGET_DIR, an open file or a tmux pane inside it (or inside the worktree of a `target` dir). Prints `dir: N bytes (idle H h)`.
set -u
# PROC_ROOT and TMUX_PANES are injected by the test: a live process table made it depend on what runs on the host.
PROC_ROOT=${PROC_ROOT:-/proc}; TMUX_PANES=${TMUX_PANES:-tmux list-panes -a -F #{pane_current_path}}
hours=72; apply=0
while [ $# -gt 0 ]; do case $1 in --hours) hours=$2; shift 2;; --apply) apply=1; shift;; *) echo "usage: $0 [--hours N] [--apply]" >&2; exit 2;; esac; done
HERE=$(cd "$(dirname "$0")" && pwd)
roots=${SWEEP_ROOTS:-"$HOME/scratch $(git -C "$HERE" worktree list --porcelain | sed -n 's/^worktree //p' | xargs -r -n1 dirname | sort -u | tr '\n' ' ')"}
now=$(date +%s)
profiles() { for p in "$1"/*/; do p=${p%/}; case $(basename "$p") in debug|release|dist|fast|incremental|tmp) echo "$p";; *) [ -d "$p/.fingerprint" ] && echo "$p";; esac; done; }
newest() { local m=0 t; for p in "$1" $(profiles "$1") ; do t=$(stat -c %Y "$p" 2>/dev/null) && [ "$t" -gt "$m" ] && m=$t; done; echo "$m"; }
in_use() {
  local d=$1 guard=$1; [ "$(basename "$d")" = target ] && guard=$(dirname "$d")
  local p
  for p in "$PROC_ROOT"/[0-9]*; do
    case $(readlink "$p/cwd" 2>/dev/null) in "$guard"|"$guard"/*) return 0;; esac
    { tr "\0" "\n" < "$p/environ"; } 2>/dev/null | grep -qx "CARGO_TARGET_DIR=$d\(/.*\)\?" && return 0
    for f in "$p"/fd/*; do case $(readlink "$f" 2>/dev/null) in "$d"/*) return 0;; esac; done 2>/dev/null
  done
  $TMUX_PANES 2>/dev/null | grep -q "^$guard\(/\|$\)" && return 0
  return 1
}
total=0
for root in $roots; do
  for d in $(find "$root" -maxdepth 4 \( -name .rustc_info.json -o -name CACHEDIR.TAG \) -printf '%h\n' 2>/dev/null | sort -u); do
    idle=$(( (now - $(newest "$d")) / 3600 )); [ "$idle" -ge "$hours" ] || continue
    in_use "$d" && { echo "$d: in use, kept"; continue; }
    size=0; for p in $(profiles "$d"); do size=$(( size + $(du -sb "$p" 2>/dev/null | cut -f1) )); done
    [ "$size" -gt 0 ] || continue
    if [ $apply = 1 ]; then
      idle=$(( (now - $(newest "$d")) / 3600 )); [ "$idle" -ge "$hours" ] && ! in_use "$d" || continue   # re-check right before deleting
      for p in $(profiles "$d"); do rm -rf -- "$p"; done
    fi
    echo "$d: $size bytes (idle $idle h)"; total=$((total + size))
  done
done
echo "total: $total bytes $([ $apply = 1 ] && echo freed || echo would be freed)"
