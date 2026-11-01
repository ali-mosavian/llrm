#!/usr/bin/env bash
# queue-runner.sh: batches up to 8 disjoint PRs of the queue build-queue.sh writes to /tmp/land-queue.txt before every batch (tools/parity_squash.py merge-batch: one smoke, halving on failure);
# a batch that does not merge lands one at a time with land.sh. Prints `N: MERGED ...` / `N: REFUSED: ...` lines
# (the format refusals-report.sh reads). Waits for any running land.sh/runner first.
. "$(dirname "${BASH_SOURCE[0]:-$0}")/branch.sh"
[ "$1" = --print-plan ] && exec python3 "$(dirname "${BASH_SOURCE[0]:-$0}")/plan.py" "${@:2}"  # read-only: before the lock, the pid file and the loop
export BUILDSLOT_BYPASS=1
mkdir -p "${LAND_TMP:-/tmp/landing}"
# every temp file of the batch (cargo, rustc, the linker, python tempfile) goes under LAND_TMP: /tmp is a tmpfs that sat at 98% of its inodes and failed links with "Cannot create temporary file"
export TMPDIR="${LAND_TMPDIR:-${LAND_TMP:-/tmp/landing}/tmp}"; mkdir -p "$TMPDIR"
# own process group, recorded for stop.sh: the only way to end the runner (never by name or pattern)
[ "$(ps -o pgid= $$ | tr -d ' ')" = "$$" ] || exec setsid "$0" "$@"
echo $$ > "${LAND_PIDFILE:-${LAND_TMP:-/tmp/landing}/land-runner.pgid}"
HERE=$(cd "$(dirname "$0")" && pwd); source "$HERE/disk-preflight.sh"; source "$HERE/open-only.sh"; source "$HERE/pick-disjoint.sh"; source "$HERE/precheck-filter.sh"; source "$HERE/landing-busy.sh"; source "$HERE/head-age.sh"; source "$HERE/starved.sh"; source "$HERE/rate-limit.sh"; source "$HERE/alone-filter.sh"; source "$HERE/solo.sh"; source "$HERE/background.sh"; source "$HERE/stamp.sh"; source "$HERE/refusal-lines.sh"; source "$HERE/urgent.sh"; source "$HERE/stale-batch.sh"; source "$HERE/tooling-current.sh"; source "$HERE/pr-state.sh"; source "$HERE/runner-reload.sh"
R=ali-mosavian/llrm; W=~/scratch/coord-ps; T=~/scratch/coord-ps-target; Q=/tmp/land-queue.txt; touch $Q
SELF_SHA=$(runner_scripts_sha "$HERE")
while true; do
  # a merged change to the runner or a helper, or to runner.env, takes effect here, between batches (same pid and process group, so stop.sh still works)
  runner_env_load "${LAND_TMP:-/tmp/landing}/runner.env"
  [ "$(runner_scripts_sha "$HERE")" = "$SELF_SHA" ] || { echo "runner scripts changed: reloading"; exec "$0" "$@"; }
  # never run beside another landing (old runner's land.sh)
  while landing_busy; do sleep 20; done
  disk_pause
  rate_limit_pause
  bq_start=$(date +%s)
  "$HERE/build-queue.sh" > $Q.new && mv $Q.new $Q || echo "build-queue failed, keeping the old queue"
  echo "$(date +%H:%M:%S) build-queue $(( $(date +%s) - bq_start )) s" >> "${LAND_TMP:-/tmp/landing}/assemble.log"
  # landings outside the gate: every first-parent commit of $LAND_BRANCH not in landed.log, reported once
  git -C "$W" fetch -q origin 2>/dev/null; (cd "$W" && "$HERE/unlogged.sh" 2>/dev/null) | while read -r l; do
    grep -qxF "$l" "${LAND_TMP:-/tmp/landing}/unlogged.log" 2>/dev/null || { echo "UNLOGGED LANDING: $l"; echo "$l" >> "${LAND_TMP:-/tmp/landing}/unlogged.log"; }
  done
  # size cap: cargo never removes the artifacts of older hashes
  [ "$(du -sk "$T" 2>/dev/null | cut -f1)" -gt "${TARGET_CAP_KB:-41943040}" ] && echo "target over cap: $(PRUNE_LOG=${LAND_TMP:-/tmp/landing}/prune.log "$HERE/prune-target.sh" "$T" --apply)"
  # every cargo target over 15 GB loses its stale hashes, at most every 10 minutes
  if [ -z "$(find "${LAND_TMP:-/tmp/landing}/sweep.stamp" -mmin -10 2>/dev/null)" ]; then
    touch "${LAND_TMP:-/tmp/landing}/sweep.stamp"; background_timeout "${SWEEP_TIMEOUT:-1800}" "${LAND_TMP:-/tmp/landing}/sweep.log" "$HERE/sweep-targets.sh"
  fi
  n=$(head -1 $Q); [ -z "$n" ] && { sleep 20; continue; }
  batch=("$n")
  BATCH_CAP=${BATCH_CAP:-48}  # PRs per batch (runner.env); the window read from the queue is 24 more
  cand=($(sed -n "1,$((BATCH_CAP + 24))p" $Q))
  # the cheap static checks (ratchets, fmt, docs, deletions) on each PR merged with $LAND_BRANCH: a failure never reaches a batch (and never halves it)
  if [ "${LAND_FAST:-}" = 1 ]; then  # the user's call: no precheck, no smoke, up to 16 PRs in queue order; merge-batch leaves out the ones that cannot join
    # only PRs that merge one after another: one that conflicts with an earlier member waits a round instead of costing the others a smoke
    # closed PRs are dropped and a head younger than 15 minutes waits (a fix may be on its way) before the assembly
    # a PR listed in solo.txt at the head of the queue is a batch of its own (the others wait one cycle)
    solo=$(solo_listed "$HERE/solo.txt" "$n"); [ -n "$solo" ] && cand=("$n")
    # PRs in $LAND_TMP/urgent.txt (landing fixes) are a batch of their own, in that order; the rest wait one cycle
    urgent=($(urgent_from_queue "${LAND_TMP:-/tmp/landing}/urgent.txt" "$Q")); [ -z "$solo" ] && [ ${#urgent[@]} -gt 0 ] && cand=("${urgent[@]}")
    echo "$(date +%H:%M:%S) queue $(wc -l < $Q) lines, window ${#cand[@]}, urgent: ${urgent[*]:-none}" >> "${LAND_TMP:-/tmp/landing}/assemble.log"
    python3 "$HERE/pr_state.py" fill "${cand[@]:0:BATCH_CAP}" 2>>"${LAND_TMP:-/tmp/landing}/assemble.log"  # one GraphQL call for the cycle's reads below
    cand=($(open_only "${cand[@]:0:BATCH_CAP}" 2>/dev/null)); cand=($(HEAD_MIN_AGE=$(batch_head_age $([ -z "$solo" ] && echo ${#urgent[@]} || echo 0)) aged "${cand[@]}" 2>>"${LAND_TMP:-/tmp/landing}/assemble.log"))
    # each PR alone on parity+PR (merge, build, fmt, scanners) under buildslot; one that fails is dropped here, with its REFUSED line, and never enters the batch
    tooling_current "$HERE/.." 2>>"${LAND_TMP:-/tmp/landing}/assemble.log" || echo "tooling not refreshed" >> "${LAND_TMP:-/tmp/landing}/assemble.log"
    cand=($(alone_filter "${cand[@]:0:BATCH_CAP}"))
    # aging: a PR left out of STARVE_K (default 6) consecutive batches is a batch of its own, as an urgent one is (O(queue) per cycle)
    SF="${LAND_TMP:-/tmp/landing}/starved.txt"; seen=("${cand[@]}"); aged_one=$([ -z "$solo" ] && [ ${#urgent[@]} -eq 0 ] && starved_next "$SF" "${cand[@]}")
    [ -n "$aged_one" ] && { echo "NOTE: #$aged_one was left out of $STARVE_K batches; it is a batch of its own" | tee -a "${LAND_TMP:-/tmp/landing}/assemble.log"; cand=("$aged_one"); }
    asm_start=$(date +%s)
    if [ -n "$solo" ]; then batch=("${cand[@]}"); else
    batch=($(python3 "$HERE/../parity_squash.py" assemble $BATCH_CAP "${cand[@]:0:BATCH_CAP + 24}" 2>>"${LAND_TMP:-/tmp/landing}/assemble.log")); [ ${#batch[@]} -eq 0 ] && batch=("${cand[@]:0:BATCH_CAP}"); fi
    asm_s=$(( $(date +%s) - asm_start ))
    [ ${#seen[@]} -gt 0 ] && starved_record "$SF" "${batch[*]}" "${seen[@]}"
  else
    cand=($(prechecked "${cand[@]:0:${PRECHECK_WINDOW:-16}}"))
    batch=($(pick_disjoint 8 "${cand[@]}"))
  fi
  [ ${#batch[@]} -eq 0 ] && { sleep 60; continue; }
  open=($(pop_open $Q "${batch[@]}"))
  batch=("${open[@]}"); [ ${#batch[@]} -eq 0 ] && continue; n=${batch[0]}
  echo "== $(date +%H:%M) landing ${batch[*]}"
  if [ ${#batch[@]} -eq 1 ]; then "$HERE/land.sh" "$n" 2>&1 | cut -c1-260; continue; fi
  for m in "${batch[@]}"; do [[ $m =~ ^[0-9]+$ ]] || continue
    info="$(cached_state $m state) $(cached_state $m isDraft) $(cached_state $m baseRefName)"; [[ $info =~ ^[A-Z]+\ (true|false)\  ]] || info=$(gh pr view $m --repo $R --json state,isDraft,baseRefName -q '"\(.state) \(.isDraft) \(.baseRefName)"'); set -- $info
    [ "$1" = OPEN ] || continue
    [ "$3" = $LAND_BRANCH ] || gh pr edit $m --repo $R --base $LAND_BRANCH >/dev/null
  done
  log=${LAND_TMP:-/tmp/landing}/sqb-${batch[0]}.log
  rm -f "$log.done"
  stale_batch_watch "$log"
  [ "${LAND_FAST:-}" = 1 ] && alone_warm_at_squash "$log" $(sed -n "1,${BATCH_CAP}p" $Q)  # warms the verdict cache while the batch squashes, not while it tests
  ( flock -n 9 || { echo "waiting for the landing lock (another landing holds the checkout)" >&2; flock -x 9; }; cd $W && git fetch -q origin && git reset -q --hard && git clean -fdq && git checkout -q --detach ${LAND_TOOLS_REF:-origin/$LAND_BRANCH} \
    && { echo "STAGE assemble ${asm_s:-0}s"; WAITS=${WAITS:-$HERE/waits.txt} PARITY_BIN=$T/release CARGO_TARGET_DIR=$T CARGO_INCREMENTAL=0 PYTHONUNBUFFERED=1 uv run --project tools python tools/parity_squash.py merge-batch "${batch[@]}"; } 2>&1 \
      | stamp_lines > $log ) 9>"$W.checkout.lock"
  touch "$log.done"
  for m in "${batch[@]}"; do [[ $m =~ ^[0-9]+$ ]] || continue
    if [ "$(landed_state $log $m)" = MERGED ]; then echo "$m: MERGED  $(gh pr view $m --repo $R --json title -q .title)"
    else echo "$m: REFUSED: batch $log: $(batch_refusal_reason "$log" "$m")"; fi
  done
done
