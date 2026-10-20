#!/usr/bin/env bash
# queue-runner.sh: batches up to 8 disjoint PRs of the queue build-queue.sh writes to /tmp/land-queue.txt before every batch (tools/parity_squash.py merge-batch: one smoke, halving on failure);
# stacked PRs (same STACKS group) and a batch that does not merge land one at a time with land.sh. Prints `N: MERGED ...` / `N: REFUSED: ...` lines
# (the format refusals-report.sh reads). Waits for any running land.sh/runner first.
export BUILDSLOT_BYPASS=1
mkdir -p "${LAND_TMP:-/tmp/landing}"
# own process group, recorded for stop.sh: the only way to end the runner (never by name or pattern)
[ "$(ps -o pgid= $$ | tr -d ' ')" = "$$" ] || exec setsid "$0" "$@"
echo $$ > "${LAND_PIDFILE:-${LAND_TMP:-/tmp/landing}/land-runner.pgid}"
HERE=$(cd "$(dirname "$0")" && pwd); source "$HERE/open-only.sh"; source "$HERE/pick-disjoint.sh"; source "$HERE/precheck-filter.sh"
R=ali-mosavian/llrm; W=~/scratch/coord-ps; T=~/scratch/coord-ps-target; Q=/tmp/land-queue.txt; touch $Q
STACKS="2752 2773 2776|2785 2789 2797|2713 2738 2764|2753 2757 2763|2672 2683"
group() { for g in ${STACKS//|/ |}; do :; done; echo "$STACKS" | tr '|' '\n' | grep -w -- "$1" | head -1; }
while true; do
  # never run beside another landing (old runner's land.sh)
  while ps -eo args | awk '/land\.sh/ && !/awk/ {f=1} END{exit !f}'; do sleep 20; done
  "$HERE/build-queue.sh" > $Q.new && mv $Q.new $Q || echo "build-queue failed, keeping the old queue"
  # landings outside the gate: every first-parent commit of feat/parity not in landed.log, reported once
  git -C "$W" fetch -q origin 2>/dev/null; (cd "$W" && "$HERE/unlogged.sh" 2>/dev/null) | while read -r l; do
    grep -qxF "$l" "${LAND_TMP:-/tmp/landing}/unlogged.log" 2>/dev/null || { echo "UNLOGGED LANDING: $l"; echo "$l" >> "${LAND_TMP:-/tmp/landing}/unlogged.log"; }
  done
  # size cap: cargo never removes the artifacts of older hashes
  [ "$(du -sk "$T" 2>/dev/null | cut -f1)" -gt "${TARGET_CAP_KB:-41943040}" ] && echo "target over cap: $(PRUNE_LOG=${LAND_TMP:-/tmp/landing}/prune.log "$HERE/prune-target.sh" "$T" --apply)"
  # every cargo target over 15 GB loses its stale hashes, at most every 10 minutes
  if [ -z "$(find "${LAND_TMP:-/tmp/landing}/sweep.stamp" -mmin -10 2>/dev/null)" ]; then
    touch "${LAND_TMP:-/tmp/landing}/sweep.stamp"; "$HERE/sweep-targets.sh" 2>&1 | sed "s/^/$(date +%H:%M) /" >> "${LAND_TMP:-/tmp/landing}/sweep.log"
  fi
  n=$(head -1 $Q); [ -z "$n" ] && { sleep 20; continue; }
  batch=("$n"); g=$(group "$n")
  if [ -z "$g" ]; then
    cand=(); for m in $(sed -n '1,40p' $Q); do [ -z "$(group "$m")" ] && cand+=("$m"); done
    # the cheap static checks (ratchets, fmt, docs, deletions) on each PR merged with feat/parity: a failure never reaches a batch (and never halves it)
    if [ "${LAND_FAST:-}" = 1 ]; then  # the user's call: no precheck, no smoke, up to 16 PRs in queue order; merge-batch leaves out the ones that cannot join
      batch=("${cand[@]:0:16}")
    else
      cand=($(prechecked "${cand[@]:0:${PRECHECK_WINDOW:-16}}"))
      batch=($(pick_disjoint 8 "${cand[@]}"))
    fi
    [ ${#batch[@]} -eq 0 ] && { sleep 60; continue; }
  fi
  for m in "${batch[@]}"; do sed -i "/^$m\$/d" $Q; done
  open=($(open_only "${batch[@]}"))
  batch=("${open[@]}"); [ ${#batch[@]} -eq 0 ] && continue; n=${batch[0]}
  echo "== $(date +%H:%M) landing ${batch[*]}"
  if [ ${#batch[@]} -eq 1 ]; then "$HERE/land.sh" "$n" 2>&1 | cut -c1-260; continue; fi
  for m in "${batch[@]}"; do
    info=$(gh pr view $m --repo $R --json state,isDraft,baseRefName -q '"\(.state) \(.isDraft) \(.baseRefName)"'); set -- $info
    [ "$1" = OPEN ] || continue
    [ "$3" = feat/parity ] || gh pr edit $m --repo $R --base feat/parity >/dev/null
    [ "$2" = true ] && gh pr ready $m --repo $R >/dev/null
  done
  log=${LAND_TMP:-/tmp/landing}/sqb-${batch[0]}.log
  ( flock -n 9 || { echo "waiting for the landing lock (another landing holds the checkout)" >&2; flock -x 9; }; cd $W && git fetch -q origin && git reset -q --hard && git clean -fdq && git checkout -q --detach ${LAND_TOOLS_REF:-origin/feat/parity} \
    && PARITY_BIN=$T/release CARGO_TARGET_DIR=$T CARGO_INCREMENTAL=0 PYTHONUNBUFFERED=1 uv run --project tools python tools/parity_squash.py merge-batch "${batch[@]}" 2>&1 \
      | awk '{print strftime("%H:%M:%S"), $0; fflush()}' > $log ) 9>"$W.checkout.lock"
  for m in "${batch[@]}"; do
    if [ "$(gh pr view $m --repo $R --json state -q .state)" = MERGED ]; then echo "$m: MERGED  $(gh pr view $m --repo $R --json title -q .title)"
    else echo "$m: REFUSED: batch $log: $(grep -E "#$m|REFUSED" $log | head -2 | tr '\n' ' ' | cut -c1-400)"; fi
  done
done
