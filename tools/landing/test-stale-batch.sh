#!/usr/bin/env bash
# Regression: the 03:03 batch ran 20+ min on the heads it read at 03:03 while two owners repushed fixes; nothing in the runner log said so, and the user had to ask.
here=$(cd "$(dirname "$0")" && pwd); source "$here/stale-batch.sh"
now=$(date +%s); d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
printf '%s\n' "11 1500 uv run --project tools python tools/parity_squash.py merge-batch 5071 5085" "12 1500 /x/python3 tools/parity_squash.py merge-batch 5071 5085" "13 100 /x/python3 tools/parity_squash.py merge-batch 7 8" > $d/ps
printf '#!/bin/sh\ncat %s\n' "$d/ps" > $d/psc; chmod +x $d/psc
printf '#!/bin/sh\ncase "$1" in 5071) echo %s;; *) echo %s;; esac\n' "$((now - 600))" "$((now - 3000))" > $d/com; chmod +x $d/com
got=$(STALE_PS=$d/psc STALE_COMMITTED=$d/com stale_batch_lines)
[ "$got" = "STALE BATCH pid 12: 25 min old, heads moved since it began: #5071" ] || { echo "FAIL: stale batch not named once with the moved head only: '$got'"; exit 1; }
printf '#!/bin/sh\necho %s\n' "$((now - 3000))" > $d/com2; chmod +x $d/com2
[ -z "$(STALE_PS=$d/psc STALE_COMMITTED=$d/com2 stale_batch_lines)" ] || { echo "FAIL: a batch whose heads did not move was named"; exit 1; }
grep -q 'stale_batch_watch' "$here/queue-runner.sh" || { echo "FAIL: queue-runner never starts the watcher"; exit 1; }
echo PASS stale-batch
