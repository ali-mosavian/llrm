#!/usr/bin/env bash
# Regression: a PR whose build fails alone must be dropped before assembly, and a check that cannot run must not stop the queue.
. "$(dirname "$0")/alone-filter.sh"; export LAND_TMP=$(mktemp -d)
d=$(mktemp -d)
printf '#!/bin/sh\nshift\nfor n; do [ "$n" = 2 ] && echo "$n: REFUSED: alone: does not build" >&2 || echo $n; done\n' > $d/drop; printf '#!/bin/sh\nexit 3\n' > $d/broken; chmod +x $d/*
ALONE_CMD="$d/drop x" ; got=$(alone_filter 1 2 3 2>/dev/null)
[ "$got" = "1 3" ] || { echo "FAIL: build-failing PR not dropped: '$got'"; exit 1; }
ALONE_CMD="$d/broken x"; got=$(alone_filter 1 2 3 2>/dev/null)
[ "$got" = "1 2 3" ] || { echo "FAIL: broken check stalled or filtered the queue: '$got'"; exit 1; }
echo PASS

# Regression: the cache warm launched parity_squash.py beside the batch whatever ALONE_CMD said, and competed with the squash for slots (batch 19:45).
w=$(mktemp -d); printf '#!/bin/sh\necho "$@" >> %s/ran\n' "$w" > $w/rec; chmod +x $w/rec
ALONE_CMD="$w/rec alone"; alone_warm 7 8; sleep 1
[ "$(cat $w/ran 2>/dev/null)" = "alone 7 8" ] || { echo "FAIL: warm did not run ALONE_CMD: '$(cat $w/ran 2>/dev/null)'"; exit 1; }
ALONE_CMD=echo; rm -f $w/ran; alone_warm 7 8; sleep 1; [ ! -e $w/ran ] || { echo "FAIL"; exit 1; }
echo PASS warm
