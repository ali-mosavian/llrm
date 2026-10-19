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
echo PASS
