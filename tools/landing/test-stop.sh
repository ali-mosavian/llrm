#!/usr/bin/env bash
# Regression: a cleanup by command-line pattern killed sessions that only mentioned "merge-batch" in their prompt. stop.sh ends the recorded runner group
# and spares a sibling whose command line matches. Nothing here kills by pattern: the pattern case only records what a pattern would select (pgrep).
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d); export LAND_PIDFILE=$d/pgid
mark="merge-batch-$$-$RANDOM"
alive() { [ -r /proc/$1/status ] && ! grep -q "^State:.Z" /proc/$1/status; }
setsid bash -c 'echo $$ > "$LAND_PIDFILE"; sleep 60 & sleep 60' >/dev/null 2>&1 &
setsid bash -c 'echo $$ > "$1"; sleep 60; :' "$mark" "$d/bystander" >/dev/null 2>&1 &
for _ in 1 2 3 4 5 6 7 8 9 10; do [ -s "$LAND_PIDFILE" ] && [ -s "$d/bystander" ] && break; sleep 0.2; done
runner=$(cat "$LAND_PIDFILE"); bystander=$(cat "$d/bystander")
would_kill=$(pgrep -f -- "$mark" | tr '\n' ' ')
case " $would_kill" in *" $bystander "*) ;; *) echo "FAIL: premise: the pattern does not select the sibling ($would_kill)"; exit 1;; esac
"$here/stop.sh" >/dev/null; sleep 0.5
alive "$runner" && { echo "FAIL: runner survived"; kill -- "-$bystander"; exit 1; }
alive "$bystander" || { echo "FAIL: a sibling matching the pattern was killed"; exit 1; }
kill -- "-$bystander"; echo PASS
