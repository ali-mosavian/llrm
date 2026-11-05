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
out=$(IDLE_HOURS=0 "$here/prune-target.sh" $d); case "$out" in *"4 entries"*) ;; *) echo "FAIL: dry run counts: $out"; exit 1;; esac
bytes=$(echo "$out" | sed -E 's/.*\(([0-9]+) bytes\).*/\1/'); [ "$bytes" -ge 50000 ] || { echo "FAIL: the incremental entry's size was not counted: $out"; exit 1; }
[ -e $d/debug/deps/libfoo-aaaaaaaaaaaaaaaa.rlib ] || { echo "FAIL: dry run removed a file"; exit 1; }
IDLE_HOURS=0 PRUNE_LOG=$d/prune.log "$here/prune-target.sh" $d --apply >/dev/null
grep -q "pruned 4 entries" $d/prune.log && grep -q "libfoo-aaaaaaaaaaaaaaaa.rlib" $d/prune.log || { echo "FAIL: the run did not log its list"; exit 1; }
for gone in debug/deps/libfoo-aaaaaaaaaaaaaaaa.rlib debug/deps/libfoo-aaaaaaaaaaaaaaaa.d debug/deps/libbar-dddddddddddddddd.rlib debug/incremental/old-1; do
  [ -e $d/$gone ] && { echo "FAIL: stale $gone survived"; exit 1; }; done
for kept in src/lib.rs .git/HEAD debug/build/script-aaaaaaaaaaaaaaaa debug/deps/libfoo-bbbbbbbbbbbbbbbb.rlib debug/deps/libdep-cccccccccccccccc.rlib debug/deps/libbar-eeeeeeeeeeeeeeee.rlib debug/incremental/fresh-1; do
  [ -e $d/$kept ] || { echo "FAIL: $kept was removed"; exit 1; }; done

# Regression (qb-metadata, 11-05 17:24Z): the sweep removed libwinnow/llrm_* rlibs a live build still needed, because only the newest-mtime hash of a unit was kept. A hash a fresh
# .fingerprint names stays; a dir touched within IDLE_HOURS, or under a running cargo/rustc, is left alone.
stale() { # DIR: libfoo H1 (live), H2 (newest), H0 (stale), all deps files old
  local t=$1; mkdir -p $t/debug/deps $t/debug/.fingerprint/foo-1111111111111111 $t/debug/.fingerprint/foo-2222222222222222 $t/debug/.fingerprint/foo-0000000000000000
  mk $t/debug/deps/libfoo-1111111111111111.rlib "10 hours ago"; mk $t/debug/deps/libfoo-2222222222222222.rlib "5 hours ago"; mk $t/debug/deps/libfoo-0000000000000000.rlib "11 hours ago"
  touch -d "10 hours ago" $t/debug/.fingerprint/foo-2222222222222222 $t/debug/.fingerprint/foo-0000000000000000; touch $t/debug/.fingerprint/foo-1111111111111111
}
l=$(mktemp -d); stale $l; IDLE_HOURS=0 "$here/prune-target.sh" $l --apply >/dev/null
[ -e $l/debug/deps/libfoo-1111111111111111.rlib ] || { echo "FAIL: the hash a fresh .fingerprint names was pruned (it is not the newest by mtime)"; exit 1; }
[ -e $l/debug/deps/libfoo-2222222222222222.rlib ] || { echo "FAIL: the newest hash was pruned"; exit 1; }
[ -e $l/debug/deps/libfoo-0000000000000000.rlib ] && { echo "FAIL: control: a stale hash with a stale fingerprint survived"; exit 1; }
b=$(mktemp -d); stale $b; touch -d "10 hours ago" $b/debug/.fingerprint/foo-1111111111111111; pr=$(mktemp -d); mkdir -p $pr/4242
echo rustc > $pr/4242/comm; ln -s $b/debug $pr/4242/cwd; : > $pr/4242/environ; : > $pr/4242/cmdline
out=$(PROC_ROOT=$pr IDLE_HOURS=0 "$here/prune-target.sh" $b --apply); case "$out" in *"in 0 entries"*) ;; *) echo "FAIL: a dir under a running rustc was pruned: $out"; exit 1;; esac
[ -e $b/debug/deps/libfoo-1111111111111111.rlib ] || { echo "FAIL: a file under a running rustc was removed"; exit 1; }
out=$(PROC_ROOT=$(mktemp -d) IDLE_HOURS=0 "$here/prune-target.sh" $b --apply); case "$out" in *"in 0 entries"*) echo "FAIL: control: with no process the stale hashes stayed: $out"; exit 1;; esac
i=$(mktemp -d); stale $i; touch -d "10 hours ago" $i/debug/.fingerprint/foo-1111111111111111; mk $i/debug/deps/libnew-3333333333333333.rlib "1 minute ago"
out=$(IDLE_HOURS=3 "$here/prune-target.sh" $i --apply); case "$out" in *"in 0 entries"*) ;; *) echo "FAIL: a dir touched a minute ago was pruned: $out"; exit 1;; esac
out=$(IDLE_HOURS=0 "$here/prune-target.sh" $i --apply); case "$out" in *"in 0 entries"*) echo "FAIL: control: IDLE_HOURS=0 pruned nothing: $out"; exit 1;; esac
echo PASS
