#!/usr/bin/env bash
# Regression: a PR whose build fails alone must be dropped before assembly, and a check that cannot run must not stop the queue.
. "$(dirname "$0")/alone-filter.sh"; export LAND_TMP=$(mktemp -d)
d=$(mktemp -d)
printf '#!/bin/sh\nshift\nfor n; do [ "$n" = 2 ] && echo "$n: REFUSED: alone: does not build" >&2 || echo $n; done\n' > $d/drop; printf '#!/bin/sh\nexit 3\n' > $d/broken; chmod +x $d/*
ALONE_CMD="$d/drop x" ; got=$(alone_filter 1 2 3 2>/dev/null)
[ "$got" = "1 3" ] || { echo "FAIL: build-failing PR not dropped: '$got'"; exit 1; }
# Regression (reversed ruling): a check that cannot run let every PR into the batch unfiltered, 55 times; it is retried once and the cycle held.
export ALONE_RETRY_WAIT=0; printf '#!/bin/sh\necho x >> %s/runs\nexit 3\n' "$d" > $d/broken; ALONE_CMD="$d/broken x"; rm -f $d/runs; got=$(alone_filter 1 2 3 2>/dev/null)
[ -z "$got" ] && [ "$(wc -l < $d/runs)" = 2 ] || { echo "FAIL: broken check was not retried once and held: '$got' runs=$(wc -l < $d/runs)"; exit 1; }
# no PR to filter: the check is not run at all (it exits 2 on no PR)
rm -f $d/runs; got=$(alone_filter 2>&1); [ -z "$got" ] && [ ! -e $d/runs ] || { echo "FAIL: an empty candidate list ran the check or printed: '$got'"; exit 1; }
echo PASS

# Regression: the cache warm launched parity_squash.py beside the batch whatever ALONE_CMD said, and competed with the squash for slots (batch 19:45).
w=$(mktemp -d); printf '#!/bin/sh\necho "$@" >> %s/ran\n' "$w" > $w/rec; chmod +x $w/rec
ALONE_CMD="$w/rec alone"; alone_warm 7 8; sleep 1
[ "$(cat $w/ran 2>/dev/null)" = "alone 7 8" ] || { echo "FAIL: warm did not run ALONE_CMD: '$(cat $w/ran 2>/dev/null)'"; exit 1; }
ALONE_CMD=echo; rm -f $w/ran; alone_warm 7 8; sleep 1; [ ! -e $w/ran ] || { echo "FAIL"; exit 1; }
echo PASS warm

# Regression: scanner text on the check's stdout ("scanners: 2 targets, 0 red (tree ...)") became PR numbers "targets," "0" "red" and the real survivors vanished: no batch for 70 min.
printf '#!/bin/sh\nshift\necho "scanners: 2 targets, 0 red (tree abc123)"\nfor n; do echo $n; done\n' > $d/junk; chmod +x $d/junk
ALONE_CMD="$d/junk x"; got=$(alone_filter 11 12 2>/dev/null)
[ "$got" = "11 12" ] || { echo "FAIL: junk on stdout reached the candidates: '$got'"; exit 1; }
echo PASS junk

# Regression: the next batch's precheck was warmed before the batch's lib-tests and took 6 of 8 cargo processes (load 108); it starts at the squash stage, and never if there is none.
w=$(mktemp -d); printf '#!/bin/sh\necho "$@" >> %s/ran\n' "$w" > $w/rec; chmod +x $w/rec
ALONE_CMD="$w/rec alone"; ALONE_WARM_POLL=0.1
: > $w/log; alone_warm_at_squash $w/log 7 8; sleep 0.5
[ ! -e $w/ran ] || { echo "FAIL: warm started before the squash stage"; exit 1; }
echo "12:00:00 STAGE lib-tests" >> $w/log; sleep 0.5
[ ! -e $w/ran ] || { echo "FAIL: warm started during the lib-tests"; exit 1; }
echo "12:09:00 STAGE squash #7" >> $w/log; sleep 1
[ "$(cat $w/ran 2>/dev/null)" = "alone 7 8" ] || { echo "FAIL: warm did not start at the squash: '$(cat $w/ran 2>/dev/null)'"; exit 1; }
rm -f $w/ran; : > $w/log2; alone_warm_at_squash $w/log2 7 8; touch $w/log2.done; sleep 0.5; echo "STAGE squash" >> $w/log2; sleep 0.5
[ ! -e $w/ran ] || { echo "FAIL: warm started after the batch ended without a squash"; exit 1; }
echo PASS warm-at-squash

# Regression: the cache warm was detached with no end of its own (3000 s per PR), so batches left alone processes that held all 6 alone slots for hours; it is bounded by ALONE_WARM_TIMEOUT.
w=$(mktemp -d); printf '#!/bin/sh\necho $$ > %s/pid\nsleep 30\n' "$w" > $w/hang; chmod +x $w/hang
ALONE_CMD="$w/hang alone"; ALONE_WARM_TIMEOUT=1; alone_warm 7; sleep 0.5; pid=$(cat $w/pid)
kill -0 "$pid" 2>/dev/null || { echo "FAIL: warm ended before its timeout"; exit 1; }
sleep 1.5; kill -0 "$pid" 2>/dev/null && { kill "$pid"; echo "FAIL: warm outlived ALONE_WARM_TIMEOUT"; exit 1; }
echo PASS warm-timeout
# bulk order: LAND_SKIP_ALONE=1 passes every PR through and runs no check (a cold alone cost ~35 min per 48 PRs)
d=$(mktemp -d); printf '#!/bin/sh\ntouch %s/ran\n' "$d" > $d/check; chmod +x $d/check
out=$(LAND_SKIP_ALONE=1 ALONE_CMD=$d/check LAND_TMP=$d alone_filter 11 12 13); [ "$out" = "11 12 13" ] && [ ! -e $d/ran ] || { echo "FAIL: LAND_SKIP_ALONE=1 should pass '11 12 13' unchecked: '$out' ran=$([ -e $d/ran ] && echo yes)"; exit 1; }
echo PASS skip-alone
