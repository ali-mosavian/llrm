#!/usr/bin/env bash
# Regression: a session's `until ... land.sh 3128 ...` watcher (command line only names land.sh) kept the runner waiting 80 minutes; it landed nothing.
# Hermetic: a fixed process table, so a real land.sh running on the host cannot change the answer.
source "$(dirname "$0")/landing-busy.sh"
d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
export LANDING_PS="cat $d/ps.txt"
printf '%s\n' 'bash -c sleep 5; : land.sh 3128' 'until grep -q x f; do sleep 1; done # land.sh 3128' > $d/ps.txt
landing_busy && { echo "FAIL: a command that only names land.sh reads as a landing"; exit 1; }
printf '%s\n' 'bash tools/landing/land.sh 3128' >> $d/ps.txt
landing_busy && echo PASS || { echo "FAIL: a running land.sh is missed"; exit 1; }
