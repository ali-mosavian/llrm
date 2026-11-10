#!/usr/bin/env bash
# red_tip ALLOW [RC]: `cargo test` output on stdin; prints "STOP: release/next red: <tests>" and returns 1 when a test failed that ALLOW does not excuse,
# or when cargo exited non-zero (RC) without naming a test (a build error is red, not green).
# ALLOW lines are `<test name> #<PR>`: a red test is excused only while its fix PR is named.
red_tip() {
  local red named
  red=$(sed -nE -e 's/^test (.*) \.\.\. FAILED$/\1/p' -e 's/^(.*) --- FAILED$/\1/p' | sort -u)
  named=$red  # cargo exits non-zero for an excused failure as well: only no named test at all is a build error
  [ -f "$1" ] && red=$(grep -vxFf <(awk '$2 ~ /^#[0-9]+$/ {print $1}' "$1") <<<"$red")
  [ -z "$named" ] && [ "${2:-0}" != 0 ] && { echo "STOP: release/next red: cargo test exited ${2} (build error or crash)"; return 1; }
  [ -z "$red" ] && return 0
  echo "STOP: release/next red: $(echo $red)"; return 1
}
# tip_test_cmd SELECTION: the gate's profile, debug: `debug_assert!`s and unoptimized code are what its smoke runs (a release run passed a tip with two red lib tests).
tip_test_cmd() { echo "cargo test -q --lib --no-fail-fast ${1:---workspace}"; }
# tip_crates FROM TO: the packages changed between two commits and everything that depends on them (tip_crates.py).
tip_crates() {
  local meta names sel; meta=$(mktemp)
  cargo metadata --no-deps --format-version 1 > "$meta" 2>/dev/null || { rm -f "$meta"; echo --workspace; return; }
  # no pipeline: a crashing git diff or tip_crates.py must not read as "nothing changed"
  names=$(git diff --name-only "$1" "$2") && sel=$(python3 "$(dirname "${BASH_SOURCE[0]}")/tip_crates.py" "$meta" <<<"$names"); local rc=$?
  rm -f "$meta"; [ $rc = 0 ] && echo "$sel"; return $rc
}
# tip_scanners DIR: the two cheap absolute checks of the tip checkout DIR, what tests/docs_in_sync.rs and tests/no_local_paths.rs read: a stale generated page, a tracked
# *.md naming a path of one machine. Prints "STOP: release/next red: ..." and returns 1 on either. The landing checks compare with the base, so a red tip was never blamed
# and six batches were refused on the same two tests (2026-11-07). The path list mirrors LOCAL in tests/no_local_paths.rs.
tip_scanners() {
  local d=$1 stale paths
  stale=$(cd "$d" && python3 tools/docs/docsbuild/build.py --check 2>&1 | grep -E 'generated docs are out of date|Contents list is stale|^  docs/' | sed 's/^ *//' | head -6 | tr '\n' ' ')
  [ -n "$stale" ] && { echo "STOP: release/next red: docs_in_sync (stale page): $stale"; return 1; }
  paths=$(cd "$d" && git grep -nE '/home/|/Users/|/tmp/|/var/folders/|/private/var/|~/scratch|~/work|~/\.cla''ude' -- '*.md' | cut -c1-120 | head -3 | tr '\n' ' ')
  [ -n "$paths" ] && { echo "STOP: release/next red: no_local_paths (a tracked .md names a path of one machine): $paths"; return 1; }
  paths=$(cd "$d" && git grep -ilE 'cla''ude|anthr''opic' | head -3 | tr '\n' ' ')
  [ -n "$paths" ] && { echo "STOP: release/next red: no_local_paths (a tracked file names the assistant): $paths"; return 1; }
  return 0
}
# tip_verdict WORKTREE TARGET_DIR ALLOW FROM TO: tests the crates changed from the last green tip FROM (empty: the workspace) to TO. Returns 0 when
# green; prints the STOP line and returns 1 when red. A crate list that could not be made is red, not "nothing changed".
tip_verdict() {
  local w=$1 t=$2 allow=$3 from=$4 to=$5 crates=--workspace out= rc=0 scan
  scan=$(tip_scanners "$w") || { echo "$scan"; return 1; }
  if [ -n "$from" ]; then
    crates=$(cd "$w" && tip_crates "$from" "$to" 2>/dev/null) || { echo "STOP: release/next red: tip_crates failed, no crate was tested"; return 1; }
  fi
  if [ -n "$crates" ]; then
    out=$( (cd "$w" && CARGO_TARGET_DIR="$t" $(tip_test_cmd "$crates") 2>&1; echo "rc=$?") ); rc=${out##*rc=}; out=${out%rc=*}
  fi
  # the root scanners (ALWAYS_SCANNERS of parity_squash.py), absolute on the tip: the landing checks are relative to the base, so once a scanner was red on the base no PR was blamed
  # (target_facts: 24 files over its baseline for 5 days, 2026-11-07).
  local scanners sout src
  scanners=$(python3 - "$(dirname "${BASH_SOURCE[0]}")/../parity_squash.py" <<'PY'
import re, sys
text = open(sys.argv[1]).read()
block = re.search(r"ALWAYS_SCANNERS = \((.*?)\n\)", text, re.S).group(1)
print(" ".join("--test " + name for name in re.findall(r'"(\w+)"', block)))
PY
  ) || { echo "STOP: release/next red: the scanner list could not be read from parity_squash.py"; return 1; }
  sout=$( (cd "$w" && CARGO_TARGET_DIR="$t" cargo test -q --no-fail-fast $scanners 2>&1; echo "rc=$?") ); src=${sout##*rc=}; sout=${sout%rc=*}
  out+=$'\n'"$sout"; [ "$rc" = 0 ] && rc=$src
  red_tip "$allow" "$rc" <<<"$out"
}
