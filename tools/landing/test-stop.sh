#!/usr/bin/env bash
# Regression: a cleanup by pattern killed sessions whose command line only mentioned "merge-batch". stop.sh ends the recorded runner group and spares them.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d); export LAND_PIDFILE=$d/pgid
setsid bash -c 'echo $$ > "$LAND_PIDFILE"; sleep 60 & sleep 60' >/dev/null 2>&1 &
bash -c 'sleep 60; :' "${MARK:-merge-batch}" & bystander=$!
for _ in 1 2 3 4 5 6 7 8 9 10; do [ -s "$LAND_PIDFILE" ] && break; sleep 0.2; done
runner=$(cat "$LAND_PIDFILE")
"${STOP:-$here/stop.sh}" >/dev/null; sleep 0.5
kill -0 "$runner" 2>/dev/null && { echo "FAIL: runner survived"; kill "$bystander"; exit 1; }
alive() { [ -r /proc/$1/status ] && ! grep -q "^State:.Z" /proc/$1/status; }
alive "$bystander" || { echo "FAIL: a sibling mentioning merge-batch was killed"; exit 1; }
kill "$bystander"; echo PASS
