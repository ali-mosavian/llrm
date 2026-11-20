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
# `cargo test -q` (the runner's command) prints `name --- FAILED`, not `test name ... FAILED`: 5 llrm-cc tests were red on 11-04 and the stop line said "build error or crash"
q='compile::tests::a --- FAILED
debug_tests::b --- FAILED
test result: FAILED. 194 passed; 2 failed'
echo "$q" | red_tip $d/none 101 >$d/o && { echo "FAIL: a quiet-format red tip did not stop"; exit 1; }
grep -q '^STOP: release/next red: compile::tests::a debug_tests::b$' $d/o || { echo "FAIL: quiet-format stop line names no tests: $(cat $d/o)"; exit 1; }
printf 'compile::tests::a #6716\ndebug_tests::b #6716\n' > $d/allow
echo "$q" | red_tip $d/allow 101 >$d/o || { echo "FAIL: excused quiet-format red tests stopped the runner: $(cat $d/o)"; exit 1; }
tip_test_cmd "-p x" | grep -q -- '--no-fail-fast' || { echo "FAIL: cargo stops at the first red crate, so the stop line misses the others"; exit 1; }
# tip-static.sh stops on a checkout whose .ll names a home path as its module, or whose file names a home directory (the tip went red on both, 11-19, with no gate in the way)
w=$(mktemp -d); git -C $w init -q; mkdir -p $w/f; printf "; ModuleID = '/ho""me/x/a.c'\n" > $w/f/a.ll; printf "p = '/Us""ers/bob/x'\n" > $w/b.toml; git -C $w add -A
"$here/tip-static.sh" $w >$d/o && { echo "FAIL: tip-static passed an .ll with a home path"; exit 1; }
grep -q 'an .ll names a path' $d/o || { echo "FAIL: .ll stop line: $(cat $d/o)"; exit 1; }
printf "; ModuleID = 'a.c'\n" > $w/f/a.ll; git -C $w add -A
"$here/tip-static.sh" $w >$d/o && { echo "FAIL: tip-static passed a file naming /Us""ers/bob"; exit 1; }
grep -q "developer's home directory" $d/o || { echo "FAIL: home stop line: $(cat $d/o)"; exit 1; }
printf "p = 'a'\n" > $w/b.toml; git -C $w add -A
"$here/tip-static.sh" $w >$d/o || { echo "FAIL: tip-static stopped a clean checkout: $(cat $d/o)"; exit 1; }
echo PASS
