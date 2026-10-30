#!/usr/bin/env bash
# Regression: 449 GB of cargo targets sat idle for days while the disk fell 20 GB per half hour. A target idle 73 h is cleaned (profile dirs only), a 71 h one and one in use are not,
# non-cargo files stay, and a dry run changes nothing.
here=$(cd "$(dirname "$0")" && pwd); r=$(mktemp -d)
mkt() { mkdir -p "$r/$1/debug/deps" "$r/$1/release"; echo '{}' > "$r/$1/.rustc_info.json"; echo log > "$r/$1/run.log"; mkdir -p "$r/$1/gate-bins"; echo bin > "$r/$1/gate-bins/llrm-c"
  head -c 5000 /dev/zero > "$r/$1/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib"; head -c 5000 /dev/zero > "$r/$1/release/x"
  touch -d "$2 hours ago" "$r/$1/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib" "$r/$1/release/x" "$r/$1/debug/deps" "$r/$1/debug" "$r/$1/release" "$r/$1"; }
mkt old73/target 73; mkt fresh71/target 71; mkt busy/target 80
( cd "$r/busy" && exec sleep 60 ) & busy=$!
trap 'kill $busy 2>/dev/null; rm -rf "$r"' EXIT
sleep 0.3
dry=$(SWEEP_ROOTS=$r "$here/clean-idle-targets.sh" --hours 72)
echo "$dry" | grep -q "old73/target: 10000 bytes" || { echo "FAIL: the 73 h target is not listed: $dry"; exit 1; }
echo "$dry" | grep -q "busy/target: in use" || { echo "FAIL: an in-use target was not reported kept: $dry"; exit 1; }
echo "$dry" | grep -q "fresh71" && { echo "FAIL: a 71 h target was listed: $dry"; exit 1; }
[ -e "$r/old73/target/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib" ] || { echo "FAIL: the dry run deleted"; exit 1; }
out=$(SWEEP_ROOTS=$r "$here/clean-idle-targets.sh" --hours 72 --apply)
[ ! -e "$r/old73/target/debug" ] && [ ! -e "$r/old73/target/$(echo release)" ] || { echo "FAIL: profile dirs of the 73 h target remain: $out"; exit 1; }
[ -e "$r/old73/target/run.log" ] && [ -e "$r/old73/target/gate-bins/llrm-c" ] && [ -e "$r/old73/target/.rustc_info.json" ] || { echo "FAIL: non-cargo files or the marker were removed"; exit 1; }
for kept in fresh71/target busy/target; do [ -e "$r/$kept/debug/deps/libu-aaaaaaaaaaaaaaaa.rlib" ] || { echo "FAIL: $kept was cleaned"; exit 1; }; done
echo "$out" | grep -q "total: 10000 bytes freed" || { echo "FAIL: bytes freed not reported: $out"; exit 1; }
echo PASS
