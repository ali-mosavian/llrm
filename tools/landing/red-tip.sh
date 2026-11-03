#!/usr/bin/env bash
# red_tip ALLOW [RC]: `cargo test` output on stdin; prints "STOP: release/next red: <tests>" and returns 1 when a test failed that ALLOW does not excuse,
# or when cargo exited non-zero (RC) without naming a test (a build error is red, not green).
# ALLOW lines are `<test name> #<PR>`: a red test is excused only while its fix PR is named.
red_tip() {
  local red
  red=$(sed -nE 's/^test (.*) \.\.\. FAILED$/\1/p' | sort -u)
  [ -f "$1" ] && red=$(grep -vxFf <(awk '$2 ~ /^#[0-9]+$/ {print $1}' "$1") <<<"$red")
  [ -z "$red" ] && [ "${2:-0}" != 0 ] && { echo "STOP: release/next red: cargo test exited ${2} (build error or crash)"; return 1; }
  [ -z "$red" ] && return 0
  echo "STOP: release/next red: $(echo $red)"; return 1
}
# tip_test_cmd SELECTION: the gate's profile, debug: `debug_assert!`s and unoptimized code are what its smoke runs (a release run passed a tip with two red lib tests).
tip_test_cmd() { echo "cargo test -q --lib ${1:---workspace}"; }
# tip_crates FROM TO: the packages changed between two commits and everything that depends on them (tip_crates.py).
tip_crates() {
  local meta; meta=$(mktemp); cargo metadata --no-deps --format-version 1 > "$meta" 2>/dev/null || { rm -f "$meta"; echo --workspace; return; }
  git diff --name-only "$1" "$2" | python3 "$(dirname "${BASH_SOURCE[0]}")/tip_crates.py" "$meta"; rm -f "$meta"
}
