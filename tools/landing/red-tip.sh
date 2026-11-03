#!/usr/bin/env bash
# red_tip ALLOW: `cargo test` output on stdin; prints "STOP: release/next red: <tests>" and returns 1 when a test failed that ALLOW does not excuse.
# ALLOW lines are `<test name> #<PR>`: a red test is excused only while its fix PR is named.
red_tip() {
  local red
  red=$(sed -nE 's/^test (.*) \.\.\. FAILED$/\1/p' | sort -u)
  [ -f "$1" ] && red=$(grep -vxFf <(awk '$2 ~ /^#[0-9]+$/ {print $1}' "$1") <<<"$red")
  [ -z "$red" ] && return 0
  echo "STOP: release/next red: $(echo $red)"; return 1
}
# tip_crates FROM TO: the packages whose directory changed between two commits (a crate's directory is its package name).
tip_crates() { git diff --name-only "$1" "$2" | sed -nE 's#^crates/[^/]+/([^/]+)/.*#\1#p' | sort -u | sed 's/^/-p /' | tr '\n' ' '; }
