#!/usr/bin/env bash
# Regression: /tmp reached 99% of its inodes and a batch failed its links with "Cannot create temporary file". Under 10% free blocks or inodes the runner waits.
here=$(cd "$(dirname "$0")" && pwd); source "$here/disk-preflight.sh"; d=$(mktemp -d)
fake_df() { printf '#!/bin/sh\nif [ "$1" = -Pi ]; then echo "Filesystem Inodes IUsed IFree IUse%% Mounted"; echo "x 100 %s 0 %s%% /tmp"; else echo "Filesystem 1K-blocks Used Available Use%% Mounted"; echo "x 100 %s 0 %s%% /tmp"; fi\n' "$2" "$2" "$3" "$3" > $d/df; chmod +x $d/df; }
PATH=$d:$PATH
fake_df -Pi 99 50; disk_low /tmp >/dev/null || { echo "FAIL: 99% of the inodes used was not reported low"; exit 1; }
fake_df -Pi 50 95; disk_low /tmp >/dev/null || { echo "FAIL: 95% of the blocks used was not reported low"; exit 1; }
fake_df -Pi 82 50; disk_low /tmp >/dev/null && { echo "FAIL: 82% inodes and 50% blocks used reported low"; exit 1; }
slept=""; sleep() { slept="$slept $1"; fake_df -Pi 50 50; }
fake_df -Pi 99 50; disk_pause /tmp >/dev/null
[ "${slept# }" = 60 ] || { echo "FAIL: a low disk should wait one 60 s round, then go on: '$slept'"; exit 1; }
grep -q "disk_pause" "$here/queue-runner.sh" || { echo "FAIL: queue-runner.sh does not call disk_pause before a batch"; exit 1; }
echo PASS disk-preflight
