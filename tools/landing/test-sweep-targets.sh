#!/usr/bin/env bash
# Regression: a 56 GB tgt-backend-two was never pruned (the sweep matched dir names); a 34 GB active target under scratch/<x>/llrm/target was never pruned (the sweep looked one level down and skipped active dirs). A cargo target over the cap is
# pruned at any of those depths; a small one and a dir that is not a cargo target are left alone.
here=$(cd "$(dirname "$0")" && pwd); r=$(mktemp -d)
mkt() { mkdir -p "$r/$1/debug/deps"; echo '{}' > "$r/$1/.rustc_info.json"
  head -c 5000 /dev/zero > "$r/$1/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib"; head -c 5000 /dev/zero > "$r/$1/debug/deps/libu-bbbbbbbbbbbbbbbb.rlib"
  touch -d "10 hours ago" "$r/$1/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib"; }
mkt big/llrm/target; mkt tgt-big; mkt shallow/target; mkt flat-target; mkt other/llrm/target
mkdir -p $r/notcargo/llrm/target/debug/deps; head -c 5000 /dev/zero > $r/notcargo/llrm/target/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib; touch -d "10 hours ago" $r/notcargo/llrm/target/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib
head -c 100000 /dev/zero > $r/big/llrm/target/debug/deps/filler.bin; head -c 100000 /dev/zero > $r/tgt-big/debug/deps/filler.bin
out=$(SWEEP_ROOTS=$r CAP_KB=60 "$here/sweep-targets.sh")
echo "$out" | grep -q "big/llrm/target: pruned" || { echo "FAIL: the nested over-cap target was not pruned: $out"; exit 1; }
[ ! -e $r/big/llrm/target/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib ] && [ -e $r/big/llrm/target/debug/deps/libu-bbbbbbbbbbbbbbbb.rlib ] || { echo "FAIL: stale hash kept or newest removed"; exit 1; }
for small in shallow/target flat-target other/llrm/target notcargo/llrm/target; do [ -e $r/$small/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib ] || { echo "FAIL: $small was pruned"; exit 1; }; done
echo "$out" | grep -q "tgt-big: pruned" || { echo "FAIL: a target not named target (tgt-big, 56 GB in the field) was not pruned: $out"; exit 1; }
[ "$(echo "$out" | wc -l)" = 2 ] || { echo "FAIL: other than the two over-cap dirs pruned: $out"; exit 1; }
# a target under the cap that has sat idle for hours is pruned; a busy one of the same size is not (139 GB of idle alone slots, every dir under the 15 GB cap)
mkt idle/target; mkt busy/target; head -c 100000 /dev/zero > $r/idle/target/debug/deps/filler.bin; head -c 100000 /dev/zero > $r/busy/target/debug/deps/filler.bin
touch -d "10 hours ago" $r/idle/target/debug/deps/filler.bin $r/idle/target/debug/deps/libu-bbbbbbbbbbbbbbbb.rlib $r/idle/target/.rustc_info.json $r/idle/target/debug/deps $r/idle/target/debug $r/idle/target
idle=$(SWEEP_ROOTS=$r CAP_KB=100000000 IDLE_CAP_KB=60 IDLE_HOURS=3 "$here/sweep-targets.sh")
echo "$idle" | grep -q "idle/target: pruned" || { echo "FAIL: an idle target under the cap was not pruned: $idle"; exit 1; }
echo "$idle" | grep -q "busy/target" && { echo "FAIL: a busy target under the cap was pruned: $idle"; exit 1; }
# a live build writes deep under target/ while the top dirs sit old: a depth-2 age pruned cool-zhukovsky (15 GB, newest file 15 min old) as 38 h idle
mkt deep/target; mkdir -p $r/deep/target/release/deps/x; head -c 100000 /dev/zero > $r/deep/target/release/deps/x/fresh.o
touch -d "10 hours ago" $r/deep/target/debug/deps $r/deep/target/debug $r/deep/target/.rustc_info.json $r/deep/target/release/deps/x $r/deep/target/release/deps $r/deep/target/release $r/deep/target
deep=$(SWEEP_ROOTS=$r CAP_KB=100000000 IDLE_CAP_KB=60 IDLE_HOURS=3 "$here/sweep-targets.sh")
echo "$deep" | grep -q "deep/target" && { echo "FAIL: a target with a fresh deep file was pruned as idle: $deep"; exit 1; }
# a quiet idle target that a process holds open is kept: a running build can sit silent for hours
mkt held/target; head -c 100000 /dev/zero > $r/held/target/debug/deps/filler.bin
touch -d "10 hours ago" $r/held/target/debug/deps/*.rlib $r/held/target/debug/deps/filler.bin $r/held/target/.rustc_info.json $r/held/target/debug/deps $r/held/target/debug $r/held/target
exec 9< $r/held/target/debug/deps/filler.bin
held=$(SWEEP_ROOTS=$r CAP_KB=100000000 IDLE_CAP_KB=60 IDLE_HOURS=3 "$here/sweep-targets.sh")
exec 9<&-
echo "$held" | grep -q "held/target" && { echo "FAIL: an idle target held open was pruned: $held"; exit 1; }
echo PASS
