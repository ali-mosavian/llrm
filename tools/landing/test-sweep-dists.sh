#!/usr/bin/env bash
# Regression: 400 pair_build dists (250 MB each) and their checkouts stood in scratch while the disk filled 36 GB an hour, and sweep-targets.sh pruned only cargo targets.
# An old dist with pair_build's `ok` marker goes (with its wt-pair checkout); a recent one, one without the marker, and one a live process has as its cwd stay; DRY=1 removes nothing.
here=$(cd "$(dirname "$0")" && pwd); r=$(mktemp -d); live=
trap '[ -n "$live" ] && kill "$live" 2>/dev/null; rm -rf "$r"' EXIT
mkd() { mkdir -p "$r/$1/dist"; echo x > "$r/$1/dist/llrm-qb"; head -c 1000 /dev/zero > "$r/$1/dist/big"; [ "$2" = ok ] && echo sums > "$r/$1/ok"; touch -d "${3:-5 hours ago}" "$r/$1/dist/llrm-qb"; }
mkd sbp-old ok; mkdir -p "$r/wt-pair-old"; mkd sbp-new ok "10 minutes ago"; mkd mine-nook no; mkd sbp-live ok; mkd sbp-854bb7efc4343d7c ok; mkd sbp-1234567890abcdef ok; mkd sbp-fedcba987654321 ok
(cd "$r/sbp-live" && exec sleep 300) & live=$!
sleep 0.3
export TIP_SHA=1234567890abcdef1234567890abcdef12345678
out=$(DRY=1 SWEEP_ROOTS=$r CAP_KB=99999999 "$here/sweep-targets.sh")
echo "$out" | grep -q "would remove: $r/sbp-old " || { echo "FAIL: a dry run does not list the old dist: $out"; exit 1; }
[ -e $r/sbp-old/dist/llrm-qb ] || { echo "FAIL: a dry run removed the old dist"; exit 1; }
out=$(SWEEP_ROOTS=$r CAP_KB=99999999 "$here/sweep-targets.sh")
[ ! -e $r/sbp-old ] && [ ! -e $r/wt-pair-old ] || { echo "FAIL: the old dist or its checkout stayed: $out"; exit 1; }
[ -e $r/sbp-new/dist/llrm-qb ] || { echo "FAIL: a 10-minute-old dist was removed"; exit 1; }
[ -e $r/mine-nook/dist/llrm-qb ] || { echo "FAIL: a dist with no ok marker (its owner's) was removed"; exit 1; }
[ -e $r/sbp-live/dist/llrm-qb ] || { echo "FAIL: a dist a live process works in was removed"; exit 1; }
echo "$out" | grep -q "kept (in use): $r/sbp-live" || { echo "FAIL: the live dist was not reported: $out"; exit 1; }
[ -e $r/sbp-854bb7efc4343d7c/dist/llrm-qb ] || { echo "FAIL: the pinned base dist (sbp-854bb7efc*) was removed"; exit 1; }
[ -e $r/sbp-1234567890abcdef/dist/llrm-qb ] || { echo "FAIL: the dist of the last landed tip was removed"; exit 1; }
[ ! -e $r/sbp-fedcba987654321 ] || { echo "FAIL: an old unrelated dist stayed"; exit 1; }
echo "$out" | grep -q "kept (pinned base)" && echo "$out" | grep -q "kept (last landed tip)" || { echo "FAIL: the two exemptions are not reported: $out"; exit 1; }
# An orphan checkout: wt-pair-X whose sbp-X is gone (the old rule removed it only in the pass that removed sbp-X: 72 of 90 stood for good, ~89 GB).
orph() { mkdir -p "$r/wt-pair-$1"; head -c 1000 /dev/zero > "$r/wt-pair-$1/f"; touch -d "${2:-5 hours ago}" "$r/wt-pair-$1"; }
orph aaaaaaaaaaaa; orph bbbbbbbbbbbb "10 minutes ago"; orph 854bb7efc4343d7c; orph 1234567890abcdef; orph livelivelive; mkd sbp-hasdist ok "10 minutes ago"; orph hasdist
(cd "$r/wt-pair-livelivelive" && exec sleep 300) & live2=$!
sleep 0.3
out=$(DRY=1 SWEEP_ROOTS=$r CAP_KB=99999999 "$here/sweep-targets.sh")
echo "$out" | grep -q "would remove orphan: $r/wt-pair-aaaaaaaaaaaa " || { echo "FAIL: a dry run does not list the orphan checkout: $out"; exit 1; }
[ -d $r/wt-pair-aaaaaaaaaaaa ] || { echo "FAIL: a dry run removed the orphan"; exit 1; }
out=$(SWEEP_ROOTS=$r CAP_KB=99999999 "$here/sweep-targets.sh")
[ ! -e $r/wt-pair-aaaaaaaaaaaa ] || { echo "FAIL: an old checkout with no dist stayed: $out"; exit 1; }
for keep in bbbbbbbbbbbb 854bb7efc4343d7c 1234567890abcdef livelivelive hasdist; do [ -d $r/wt-pair-$keep ] || { echo "FAIL: wt-pair-$keep (recent, pinned, tip, in use, or with its dist) was removed"; exit 1; }; done
kill $live2 2>/dev/null
echo PASS
