#!/usr/bin/env bash
# stage-sampler.sh: logs `HH:MM:SS stage` whenever the landing's stage changes. Only processes UNDER a running parity_squash root count (ROOT_PATTERN): a host-wide
# grep logged another session's `cargo build --profile dist` as the landing's build (5 min 12 s that were not the landing's).
# The root is the pid parity_squash recorded in land_lock (/tmp/land.lock.pid), waited on with kill -0: a `pgrep -f PATTERN` also matches any session whose command line names it.
ROOT_PIDFILE=${ROOT_PIDFILE:-${LAND_LOCK:-/tmp/land.lock}.pid}
descendants() { local pid=$1 c; echo "$pid"; for c in $(pgrep -P "$pid" 2>/dev/null); do descendants "$c"; done; }
sample_stage() {
  local roots r d p=""
  roots=""; [ -s "$ROOT_PIDFILE" ] && kill -0 "$(cat "$ROOT_PIDFILE")" 2>/dev/null && roots=$(cat "$ROOT_PIDFILE")
  for r in $roots; do for d in $(descendants "$r"); do p="$p"$'\n'"$(ps -o args= -p "$d" 2>/dev/null)"; done; done
  if   echo "$p" | grep -q "cargo build --profile dist"; then echo build-dist
  elif echo "$p" | grep -q "cargo build --release";       then echo build-release
  elif echo "$p" | grep -q "cargo check";                 then echo cargo-check
  elif echo "$p" | grep -q "cargo test";                  then echo cargo-test
  elif echo "$p" | grep -q "parity_smoke.py";             then echo smoke
  elif echo "$p" | grep -q "gh pr merge\|git push";       then echo squash-push
  elif [ -n "$roots" ];                                    then echo squash-other
  else echo idle; fi
}
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  last=""
  while true; do s=$(sample_stage); [ "$s" != "$last" ] && { echo "$(date +%H:%M:%S) $s"; last=$s; }; sleep 3; done
fi
