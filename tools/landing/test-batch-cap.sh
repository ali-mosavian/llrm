#!/usr/bin/env bash
# Regression: the batch cap was the literal 16 in four places of the runner and MAX_BATCH in Python; 14-16 PR batches cost ~100-130 s of fixed stages per round and ~7 s per PR, so 32 per batch halves the fixed cost per PR. One value, BATCH_CAP (default 32).
here=$(cd "$(dirname "$0")" && pwd)
! grep -nE ':0:16\}|assemble 16 |1,16p' "$here/queue-runner.sh" || { echo "FAIL: a literal 16 is left in queue-runner.sh"; exit 1; }
[ "$(grep -c 'BATCH_CAP' "$here/queue-runner.sh")" -ge 5 ] || { echo "FAIL: queue-runner does not use BATCH_CAP in its slices"; exit 1; }
echo PASS batch-cap
