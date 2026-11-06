#!/bin/bash
# load_guard.sh CMD...: admission, then CMD under a load guard.
#   admission: take one of LG_SLOTS (3) flock slots in LG_SLOT_DIR and wait for 1m load < LG_START_1M (25); LG_NO_WAIT=1 exits 75 at once when no slot is free.
#   guard:     CMD runs in its own session; every process group of its descendants is SIGSTOPped at 5m load > LG_STOP_5M (28) or 1m > LG_STOP_1M (38) and
#              exactly the recorded list is SIGCONTed at 5m < LG_RESUME_5M (26). The EXIT trap CONTs the list; INT/TERM also pass TERM to the group; a watchdog
#              in its own session CONTs the list if this wrapper is SIGKILLed (a guard left a build stopped for 67 min; a stopper that signalled one group left
#              the wrapper subshell and the `| tail -1` pipe stopped). Only recorded pids/pgids are signalled, never a pattern. Lines carry `date -u`.
# Env: LG_LOAD_FILE (/proc/loadavg), LG_POLL seconds (15), LG_SLOT_DIR (/tmp/buildslots/load_guard).
[ $# -gt 0 ] || { echo "usage: load_guard.sh CMD..." >&2; exit 2; }
file=${LG_LOAD_FILE:-/proc/loadavg}; poll=${LG_POLL:-15}
stop5=${LG_STOP_5M:-28}; stop1=${LG_STOP_1M:-38}; resume5=${LG_RESUME_5M:-26}; start1=${LG_START_1M:-25}
slots=${LG_SLOTS:-3}; slot_dir=${LG_SLOT_DIR:-/tmp/buildslots/load_guard}
log() { echo "load_guard: $(date -u +%H:%MZ) $*" >&2; }
above() { awk -v a="$1" -v b="$2" 'BEGIN{exit !(a>b)}'; }

# ---- admission -------------------------------------------------------------
admit() {
  mkdir -p "$slot_dir" || return 1
  while :; do
    for i in $(seq 1 "$slots"); do
      exec {slot_fd}>"$slot_dir/slot.$i"
      if flock -n "$slot_fd"; then
        read -r l1 _ < "$file"
        if above "$start1" "$l1"; then log "admit slot $i (1m $l1)"; return 0; fi
        flock -u "$slot_fd"
      fi
      exec {slot_fd}>&-
    done
    [ "${LG_NO_WAIT:-}" = 1 ] && { log "no free slot or load too high"; return 75; }
    sleep "$poll"
  done
}
admit || exit $?

# ---- the process tree ------------------------------------------------------
tree_pgids() {  # tree_pgids ROOT: the process group of ROOT and of every descendant
  ps -eo pid=,ppid=,pgid= | awk -v root="$1" '
    { ppid[$1] = $2; pg[$1] = $3 }
    END { seen[root] = 1; n = 1; queue[1] = root; out[pg[root]] = 1
          for (i = 1; i <= n; i++) for (p in ppid) if (ppid[p] == queue[i] && !(p in seen)) { seen[p] = 1; queue[++n] = p; out[pg[p]] = 1 }
          for (g in out) print g }'
}
state=$(mktemp)
setsid "$@" {slot_fd}>&- & child=$!
stopped=0
signal_list() { for g in $(cat "$state"); do kill "-$1" -- "-$g" 2>/dev/null; done; }
stop_tree() {
  : > "$state"
  for _ in 1 2 3; do  # a child forked between the listing and the STOP is caught on the next pass
    for g in $(tree_pgids "$child"); do grep -qx "$g" "$state" || { echo "$g" >> "$state"; kill -STOP -- "-$g" 2>/dev/null; }; done
  done
}
resume_tree() { signal_list CONT; }
cleanup() { resume_tree; rm -f "$state"; }
trap 'cleanup; kill -TERM -- "-$child" 2>/dev/null; log "signalled: groups continued, command terminated"; exit 130' INT TERM
trap 'cleanup' EXIT
# watchdog: survives a SIGKILL of this wrapper and continues the recorded groups
setsid bash -c 'while kill -0 "$1" 2>/dev/null; do sleep 1; done; for g in $(cat "$2" 2>/dev/null); do kill -CONT -- "-$g" 2>/dev/null; done' _ "$$" "$state" >/dev/null 2>&1 &

while kill -0 "$child" 2>/dev/null; do
  read -r l1 l5 _ < "$file"
  if [ $stopped = 0 ] && { above "$l5" "$stop5" || above "$l1" "$stop1"; }; then
    stop_tree; stopped=1; log "stop groups $(tr '\n' ' ' < "$state")(1m $l1, 5m $l5)"
  elif [ $stopped = 1 ] && above "$resume5" "$l5"; then
    resume_tree; stopped=0; log "resume groups $(tr '\n' ' ' < "$state")(1m $l1, 5m $l5)"
  fi
  sleep "$poll"
done
wait "$child"; rc=$?
log "release (rc $rc)"
exit $rc
