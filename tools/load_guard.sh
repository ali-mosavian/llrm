#!/bin/bash
# load_guard.sh CMD...: run CMD in its own process group; SIGSTOP the group at 5m load > 28 or 1m > 38, SIGCONT at 5m < 26.
# The EXIT trap always CONTs; INT/TERM also pass TERM to the group. A watchdog in its own session CONTs the group if this wrapper dies (SIGKILL), so no group stays in state T.
# Only the recorded pgid is signalled, never a pattern. Every stop/resume line carries `date -u`.
# Env: LG_LOAD_FILE (default /proc/loadavg), LG_POLL seconds (15), LG_STOP_5M (28), LG_STOP_1M (38), LG_RESUME_5M (26).
[ $# -gt 0 ] || { echo "usage: load_guard.sh CMD..." >&2; exit 2; }
file=${LG_LOAD_FILE:-/proc/loadavg}; poll=${LG_POLL:-15}
stop5=${LG_STOP_5M:-28}; stop1=${LG_STOP_1M:-38}; resume5=${LG_RESUME_5M:-26}
log() { echo "load_guard: $(date -u +%H:%MZ) $*" >&2; }
setsid "$@" & pgid=$!
stopped=0
cont() { kill -CONT -- "-$pgid" 2>/dev/null; }
trap 'cont; kill -TERM -- "-$pgid" 2>/dev/null; log "signalled: group $pgid continued and terminated"; exit 130' INT TERM
trap 'cont' EXIT
# dead-man: the wrapper may be SIGKILLed; this survives it and continues the group
setsid bash -c 'while kill -0 '"$$"' 2>/dev/null; do sleep 1; done; kill -CONT -- -'"$pgid"' 2>/dev/null' >/dev/null 2>&1 &
while kill -0 "$pgid" 2>/dev/null; do
  read -r l1 l5 _ < "$file"
  if [ $stopped = 0 ] && { [ "$(awk -v a="$l5" -v b="$stop5" 'BEGIN{print (a>b)}')" = 1 ] || [ "$(awk -v a="$l1" -v b="$stop1" 'BEGIN{print (a>b)}')" = 1 ]; }; then
    kill -STOP -- "-$pgid" 2>/dev/null && { stopped=1; log "stop pgid $pgid (1m $l1, 5m $l5)"; }
  elif [ $stopped = 1 ] && [ "$(awk -v a="$l5" -v b="$resume5" 'BEGIN{print (a<b)}')" = 1 ]; then
    cont; stopped=0; log "resume pgid $pgid (1m $l1, 5m $l5)"
  fi
  sleep "$poll"
done
wait "$pgid"
