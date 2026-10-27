#!/usr/bin/env bash
# Regression: the runner cut every batch at a literal 16 (window 40); the user wants bulk batches. BATCH_CAP (default 48, runner.env) is the one number, and no literal 16 or 40 is left in the batch path.
here=$(cd "$(dirname "$0")" && pwd); f=$here/queue-runner.sh
grep -q 'BATCH_CAP=${BATCH_CAP:-48}' $f || { echo "FAIL: BATCH_CAP defaults to 48"; exit 1; }
grep -qE ':0:16\}|assemble 16|1,16p|1,40p|:0:40\}' $f && { echo "FAIL: a literal batch size is left: $(grep -nE ':0:16\}|assemble 16|1,16p|1,40p|:0:40\}' $f | head -2)"; exit 1; }
echo PASS
