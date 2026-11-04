#!/usr/bin/env bash
# Regression: the runner landed a batch of up to 48 PRs in one merge push; with SERIAL=1 it lands one PR per pass (one squash commit each) and prints BASE LANDED #N.
here=$(cd "$(dirname "$0")" && pwd); . "$here/serial.sh"
[ "$(SERIAL=1 serial_batch 5 6 7)" = 5 ] || { echo "FAIL: SERIAL=1 did not keep only the first PR"; exit 1; }
[ "$(SERIAL= serial_batch 5 6 7)" = "5 6 7" ] || { echo "FAIL: the batch changed without SERIAL=1"; exit 1; }
[ -z "$(SERIAL=1 serial_batch)" ] || { echo "FAIL: an empty batch gained a member"; exit 1; }
grep -q 'serial_batch' "$here/queue-runner.sh" || { echo "FAIL: the runner does not use serial_batch"; exit 1; }
grep -q 'BASE LANDED' "$here/land.sh" && grep -q 'BASE LANDED' "$here/queue-runner.sh" || { echo "FAIL: a landing does not print BASE LANDED #N (single path and batch path)"; exit 1; }
echo PASS
