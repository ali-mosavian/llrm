#!/usr/bin/env bash
# Regression: /tmp (tmpfs) reached 98% of its inodes and links failed with "Cannot create temporary file" while the runner, which left TMPDIR alone, kept building there. The runner exports
# TMPDIR under LAND_TMP before it starts anything.
here=$(cd "$(dirname "$0")" && pwd); f=$here/queue-runner.sh
line=$(grep -n '^export TMPDIR="${LAND_TMPDIR:-${LAND_TMP:-/tmp/landing}/tmp}"' $f | head -1 | cut -d: -f1)
[ -n "$line" ] || { echo "FAIL: queue-runner.sh does not export TMPDIR under LAND_TMP"; exit 1; }
first=$(grep -nE '^(HERE=|while true|python3|cargo)' $f | head -1 | cut -d: -f1)
[ "$line" -lt "$first" ] || { echo "FAIL: TMPDIR is exported at line $line, after the runner starts work (line $first)"; exit 1; }
echo PASS
