#!/usr/bin/env bash
# Regression: three kills by pattern in one day (pkill -f merge-batch, a ps|grep|xargs kill, a pkill in a test) took down sessions whose prompts only mentioned the marker. The scanner
# flags every form, and passes the recorded pid/pgid forms.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d)
flag() { printf '%s\n' "$1" > $d/x.sh; "$here/check-kills.sh" $d/x.sh >/dev/null && { echo "FAIL: not flagged: $1"; exit 1; }; }
pass() { printf '%s\n' "$1" > $d/x.sh; "$here/check-kills.sh" $d/x.sh >/dev/null || { echo "FAIL: flagged: $1"; exit 1; }; }
flag 'pkill -f marker'
flag 'killall cargo'
flag 'ps -eo pid,args | grep x | awk "{print \$1}" | xargs -r kill'
flag 'pgrep -f marker | xargs kill'
flag 'kill $(pgrep -f marker)'
flag 'kill -- -$(pgrep -f marker)'
flag 'kill -- -$(ps -o pgid= $x)'
pass 'kill -- "-$(cat "$PIDFILE")"'
pass 'kill "$(cat "$PIDFILE")"'
pass 'kill "$pid"'
pass 'would_kill=$(pgrep -f -- "$mark" | tr "\n" " ")'
pass '# pkill is forbidden here'
echo PASS
