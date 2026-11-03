#!/usr/bin/env bash
# Regression: a red test on the release/next tip did not stop the runner; batches landed on a tip already broken.
here=$(cd "$(dirname "$0")" && pwd)
source "$here/red-tip.sh"
d=$(mktemp -d)
out='test a::ok ... ok
test z80::stale ... FAILED
test m68k::other ... FAILED'
echo "$out" | red_tip $d/none >$d/o && { echo "FAIL: a red tip did not stop"; exit 1; }
grep -q '^STOP: release/next red: m68k::other z80::stale$' $d/o || { echo "FAIL: stop line: $(cat $d/o)"; exit 1; }
printf 'z80::stale #6673\nm68k::other\n' > $d/allow
echo "$out" | red_tip $d/allow >$d/o && { echo "FAIL: an allow line without a PR excused m68k::other"; exit 1; }
grep -q 'm68k::other$' $d/o && ! grep -q 'z80::stale' $d/o || { echo "FAIL: excused set: $(cat $d/o)"; exit 1; }
printf 'z80::stale #6673\nm68k::other #6680\n' > $d/allow
echo "$out" | red_tip $d/allow >$d/o || { echo "FAIL: excused red tests stopped the runner"; exit 1; }
echo 'test a::ok ... ok' | red_tip $d/none || { echo "FAIL: a green tip stopped"; exit 1; }
# a debug_assert-only failure: the command runs the gate's debug profile
tip_test_cmd "-p x" | grep -q -- '--release' && { echo "FAIL: the tip is tested in release, where debug_assert is compiled out"; exit 1; }
# a build error names no test: cargo exited 101, which is red
echo 'error: could not compile' | red_tip $d/none 101 >$d/o && { echo "FAIL: a build error marked the tip green"; exit 1; }
grep -q 'cargo test exited 101' $d/o || { echo "FAIL: build error stop line: $(cat $d/o)"; exit 1; }
echo PASS
