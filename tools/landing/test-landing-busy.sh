#!/usr/bin/env bash
# Regression: a session's `until ... land.sh 3128 ...` watcher (command line only names land.sh) kept the runner waiting 80 minutes; it landed nothing.
source "$(dirname "$0")/landing-busy.sh"
d=$(mktemp -d); printf '#!/bin/sh\nsleep 5\n' > $d/land.sh; chmod +x $d/land.sh
bash -c 'sleep 5; : land.sh 3128' & w=$!
landing_busy && { echo "FAIL: a command that only names land.sh reads as a landing"; kill $w; exit 1; }
$d/land.sh & l=$!; sleep 0.3
landing_busy && echo PASS || { echo "FAIL: a running land.sh is missed"; kill $w $l; exit 1; }
kill $w $l 2>/dev/null
