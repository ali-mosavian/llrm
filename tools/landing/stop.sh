#!/usr/bin/env bash
# stop.sh: ends the landing runner and its batch, by the process group queue-runner.sh recorded at launch. Never by name or pattern:
# a kill by command-line pattern also matches every session whose prompt mentions it (killed splitkit-port, lir-cleanup-port, lir-text-prs-1893).
PIDFILE=${LAND_PIDFILE:-${LAND_TMP:-/tmp/landing}/land-runner.pgid}
[ -s "$PIDFILE" ] || { echo "no runner recorded in $PIDFILE"; exit 1; }
pgid=$(cat "$PIDFILE"); kill -TERM -- "-$pgid" 2>/dev/null && echo "stopped process group $pgid"; rm -f "$PIDFILE"
