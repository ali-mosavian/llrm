#!/bin/bash
# rerun.sh HEAD_SHA BASE_SHA [LEVELS...]  -- head-vs-base bench of llrm-cc, one command.
# Builds each sha once (<=2 builds), records the llrm-cc md5, refuses to time above load 45.
# Needs LLRM_PIN_CLANG and a repo checkout holding the sha (REPO, default: this checkout; W = work dir, default next to it).
# Output: $OUT/<head12>-<base12>.<level>.txt, header = shas, md5s, load.
set -euo pipefail
HEAD=${1:?head sha}; BASE=${2:?base sha}; shift 2; LEVELS=("${@:-O2 O1 O3 Os}")
[ ${#LEVELS[@]} -eq 1 ] && read -ra LEVELS <<<"${LEVELS[0]}"
P=$(cd "$(dirname "$0")" && pwd); REPO=${REPO:-$(git -C "$P" rev-parse --show-toplevel)}
W=${W:-$(dirname "$REPO")/llrm-bench}; OUT=${OUT:-$W/results}; export BENCH_WORK=$W/work; : "${LLRM_PIN_CLANG:?}"
mkdir -p "$W/bin" "$OUT"
load() { sysctl -n vm.loadavg | tr -d '{}' | awk '{print $1}'; }

build() { # sha -> $W/bin/<sha12>/ (llrm-cc + siblings); idempotent, md5 in cc.md5
  local sha; sha=$(git -C "$REPO" rev-parse "$1^{commit}"); local s=${sha:0:12} d=$W/bin/${sha:0:12}
  if [ -x "$d/llrm-cc" ] && [ -f "$d/cc.md5" ] && [ -d "$W/src-$s" ] && [ "$(md5 -q "$d/llrm-cc")" = "$(cat "$d/cc.md5")" ]; then echo "$s"; return; fi
  local wt=$W/src-$s; rm -rf "$wt"; git -C "$REPO" worktree prune; git -C "$REPO" worktree add --detach "$wt" "$sha" >&2
  (cd "$wt" && CARGO_TARGET_DIR=$W/t-$s cargo build -j4 --bin llrm-cc --bin llrm-arm64-llc >&2) || { echo "build of $s failed" >&2; exit 1; }   # set -e is off inside $(): exit explicitly
  mkdir -p "$d"; for f in llrm-cc llrm-arm64-llc; do cp "$W/t-$s/debug/$f" "$d/$f" || exit 1; done
  md5 -q "$d/llrm-cc" > "$d/cc.md5"
  rm -rf "$W/t-$s"   # one build's target is ~4 GB; the source tree stays: llrm-cc reads its data files from the path it was built in
  echo "$s"
}
L=$(load); awk -v l="$L" 'BEGIN{exit !(l>45)}' && { echo "load $L > 45: no launch" >&2; exit 3; }
H=$(build "$HEAD"); B=$(build "$BASE")
HM=$(cat "$W/bin/$H/cc.md5"); BM=$(cat "$W/bin/$B/cc.md5")
[ "$HM" != "$BM" ] || { echo "head and base llrm-cc are byte-identical ($HM): nothing to compare" >&2; exit 2; }
L=$(load); awk -v l="$L" 'BEGIN{exit !(l>45)}' && { echo "load $L > 45: no timing table" >&2; exit 3; }
PROGS=${PROGS:-$(cd "$REPO" && for p in $(ls bench); do [ "$p" != grep ] && [ -f "bench/$p/$p.c" ] && [ -f "bench/$p/$p.out" ] && echo "$p"; done; echo grep)}
for lv in "${LEVELS[@]}"; do
  f=$OUT/$H-$B.$lv.txt
  { echo "head $H md5 $HM | base $B md5 $BM | load $(load)"
    LLRM_CC_HEAD=$W/bin/$H/llrm-cc LLRM_CC_BASE=$W/bin/$B/llrm-cc BENCH_REPO=$REPO \
      python3 "$P/bench2.py" "$lv" $PROGS; } > "$f"
  echo "$f"
done
