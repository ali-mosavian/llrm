#!/usr/bin/env bash
# Regression: the runner ran sweep-targets.sh inline and a 17 minute sweep held the batch back twice; it must return at once and be ended by its timeout.
. "$(dirname "$0")/background.sh"; d=$(mktemp -d)
printf '#!/bin/sh\necho started\nsleep 30\necho finished\n' > $d/slow; chmod +x $d/slow
t0=$(date +%s); background_timeout 2 $d/log $d/slow; took=$(( $(date +%s) - t0 ))
[ "$took" -le 1 ] || { echo "FAIL: the call waited $took s"; exit 1; }
sleep 4
grep -q started $d/log && ! grep -q finished $d/log || { echo "FAIL: log is '$(cat $d/log)'"; exit 1; }
echo PASS
