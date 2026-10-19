#!/usr/bin/env bash
# Regression: the landing target dir grew to 66 GB (60 GB in debug/) because cargo never removes the artifacts of older hashes. Stale hashes of a unit go; the newest hash,
# a dependency with a single (old) hash and fresh files stay.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d); mkdir -p $d/debug/deps $d/debug/incremental/old-1 $d/debug/incremental/fresh-1
mk() { head -c 1000 /dev/zero > "$1"; touch -d "$2" "$1"; }
mk $d/debug/deps/libfoo-aaaaaaaaaaaaaaaa.rlib "10 hours ago"; mk $d/debug/deps/libfoo-bbbbbbbbbbbbbbbb.rlib "5 hours ago"
mk $d/debug/deps/libfoo-aaaaaaaaaaaaaaaa.d "10 hours ago"; mk $d/debug/deps/libdep-cccccccccccccccc.rlib "30 hours ago"
mk $d/debug/deps/libbar-dddddddddddddddd.rlib "10 hours ago"; mk $d/debug/deps/libbar-eeeeeeeeeeeeeeee.rlib "1 minute ago"
touch -d "10 hours ago" $d/debug/incremental/old-1; head -c 50000 /dev/zero > $d/debug/incremental/old-1/blob; touch -d "10 hours ago" $d/debug/incremental/old-1/blob $d/debug/incremental/old-1
mkdir -p $d/src $d/.git $d/debug/build; echo fn > $d/src/lib.rs; echo ref > $d/.git/HEAD; echo out > $d/debug/build/script-aaaaaaaaaaaaaaaa; touch -d "30 hours ago" $d/src/lib.rs $d/.git/HEAD $d/debug/build/script-aaaaaaaaaaaaaaaa
out=$("$here/prune-target.sh" $d); case "$out" in *"4 entries"*) ;; *) echo "FAIL: dry run counts: $out"; exit 1;; esac
bytes=$(echo "$out" | sed -E 's/.*\(([0-9]+) bytes\).*/\1/'); [ "$bytes" -ge 50000 ] || { echo "FAIL: the incremental entry's size was not counted: $out"; exit 1; }
[ -e $d/debug/deps/libfoo-aaaaaaaaaaaaaaaa.rlib ] || { echo "FAIL: dry run removed a file"; exit 1; }
PRUNE_LOG=$d/prune.log "$here/prune-target.sh" $d --apply >/dev/null
grep -q "pruned 4 entries" $d/prune.log && grep -q "libfoo-aaaaaaaaaaaaaaaa.rlib" $d/prune.log || { echo "FAIL: the run did not log its list"; exit 1; }
for gone in debug/deps/libfoo-aaaaaaaaaaaaaaaa.rlib debug/deps/libfoo-aaaaaaaaaaaaaaaa.d debug/deps/libbar-dddddddddddddddd.rlib debug/incremental/old-1; do
  [ -e $d/$gone ] && { echo "FAIL: stale $gone survived"; exit 1; }; done
for kept in src/lib.rs .git/HEAD debug/build/script-aaaaaaaaaaaaaaaa debug/deps/libfoo-bbbbbbbbbbbbbbbb.rlib debug/deps/libdep-cccccccccccccccc.rlib debug/deps/libbar-eeeeeeeeeeeeeeee.rlib debug/incremental/fresh-1; do
  [ -e $d/$kept ] || { echo "FAIL: $kept was removed"; exit 1; }; done
echo PASS
