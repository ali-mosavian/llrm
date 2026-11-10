#!/bin/bash
# load_guard.sh CMD...: admission, then CMD under a load guard.
#   admission: both must hold: fewer than the slot count of jobs running, and 1m load < LG_START_1M (25; LG_QUIET=1: < LG_QUIET_1M, 20, for clock-measuring jobs).
#              Slot count follows load, with hysteresis: LG_SLOTS (3) base, LG_SLOTS_HIGH (5) once 1m < LG_WIDEN_1M (20), back to base at 1m >= LG_NARROW_1M (22), unchanged between.
#              Running jobs are never stopped by the slot rule. One admission per LG_SETTLE seconds (60: a started build moves the load only after a while); LG_NO_WAIT=1 exits 75 at once when none is possible.
#   guard:     CMD runs in its own session; every process group of its descendants is SIGSTOPped at 5m load > LG_STOP_5M (28) or 1m > LG_STOP_1M (38) and
#              exactly the recorded list is SIGCONTed at 5m < LG_RESUME_5M (26) or 1m < LG_RESUME_1M (20): the 5m average lags a load that has already fallen. The EXIT trap CONTs the list; INT/TERM also pass TERM to the group; a watchdog
#              in its own session CONTs the list if this wrapper is SIGKILLed (a guard left a build stopped for 67 min; a stopper that signalled one group left
#              the wrapper subshell and the `| tail -1` pipe stopped). Only recorded pids/pgids are signalled, never a pattern. Lines carry `date -u`.
# 11-08 10:1x (user: loosen the guards; 32 cores, 230 GB free): thresholds, slots and settle raised, tier aging 5 min. Backup: load_guard.sh.bak-loosen-1108.
#   order:     a job declares LG_TIER=n (1 miscompile builds, 2 pass PRs, 3 shared-pass A/Bs, 4 benches/matrix; default 4). Waiters are listed in LG_PRIO_FILE;
#              (LG_NO_WAIT jobs skip the order) a waiter takes no free slot while a waiter of a lower tier, or of its tier that started waiting earlier, is alive.
#              The settle period is LG_SETTLE_FAST (15) while 1m < LG_FAST_1M (22) and a slot is free: the load rises past 15 with every admission, so a lower bound left the queue draining at one per LG_SETTLE.
#              Slots reach LG_SLOTS_MAX (7) while 1m < LG_WIDE_1M (22) and MemAvailable (LG_MEMINFO, /proc/meminfo) >= LG_MEM_MIN_GB (40); otherwise the 3/5 count above.
# Env: LG_LOAD_FILE (/proc/loadavg), LG_POLL seconds (5), LG_SLOT_DIR (/tmp/buildslots/load_guard).
[ $# -gt 0 ] || { echo "usage: load_guard.sh CMD... | --cancel JOBID" >&2; exit 2; }
# --cancel JOBID: kills exactly the process group recorded for that job (slot_dir/jobs/JOBID: "wrapper pgid command"), never by pattern. Refuses an
# unknown or finished id, and a record whose group leader is no longer a child of its wrapper (a reused pid is not the job).
if [ "$1" = --cancel ]; then
  jobs_dir=${LG_SLOT_DIR:-/tmp/buildslots/load_guard}/jobs; id=${2:-}
  case $id in ''|*[!0-9]*) echo "load_guard: --cancel needs a job id (digits)" >&2; exit 2 ;; esac
  [ -f "$jobs_dir/$id" ] || { echo "load_guard: no recorded job $id" >&2; exit 1; }
  read -r wrapper pgid command < "$jobs_dir/$id"
  if ! kill -0 "$wrapper" 2>/dev/null || [ "$(ps -o ppid= -p "$pgid" 2>/dev/null | tr -d ' ')" != "$wrapper" ] || [ "$(ps -o pgid= -p "$pgid" 2>/dev/null | tr -d ' ')" != "$pgid" ]; then
    rm -f "$jobs_dir/$id"; echo "load_guard: job $id has finished (its record is stale)" >&2; exit 1
  fi
  kill -CONT -- "-$pgid" 2>/dev/null; kill -TERM -- "-$pgid" 2>/dev/null
  echo "load_guard: cancelled job $id: process group $pgid ($command)"; exit 0
fi
file=${LG_LOAD_FILE:-/proc/loadavg}; poll=${LG_POLL:-5}
stop5=${LG_STOP_5M:-50}; stop1=${LG_STOP_1M:-60}; resume5=${LG_RESUME_5M:-44}; resume1=${LG_RESUME_1M:-40}; start1=${LG_START_1M:-48}
[ "${LG_QUIET:-}" = 1 ] && start1=${LG_QUIET_1M:-20}
base=${LG_SLOTS:-20}; high=${LG_SLOTS_HIGH:-20}; wide=${LG_SLOTS_MAX:-20}; widest=${LG_SLOTS_WIDEST:-24}; widest1=${LG_WIDEST_1M:-30}; widen1=${LG_WIDEN_1M:-36}; narrow1=${LG_NARROW_1M:-42}
slot_dir=${LG_SLOT_DIR:-/tmp/buildslots/load_guard}
prio=${LG_PRIO_FILE:-$slot_dir/prio}; tier=${LG_TIER:-4}
log() { echo "load_guard: $(date -u +%H:%MZ) $*" >&2; }
# An untiered call is tier 4 and starved a wasm A/B for 19 h. Warn and log it (LG_UNTIERED_LOG); LG_REQUIRE_TIER=1 refuses it (exit 2).
# User 2026-11-07 08:40Z: untiered calls stay allowed (tier 4), warned and logged; LG_REQUIRE_TIER stays 0.
if [ -z "${LG_TIER:-}" ]; then
  log "WARNING: LG_TIER not set, running as tier 4; set LG_TIER (1 miscompile, 2 pass PR, 3 A/B, 4 bench)"
  echo "$(date -u +%FT%TZ) pid=$$ cwd=$PWD caller=$(ps -o args= -p $PPID 2>/dev/null | cut -c1-120) cmd=$*" >> "${LG_UNTIERED_LOG:-$slot_dir/untiered.log}" 2>/dev/null
  [ "${LG_REQUIRE_TIER:-0}" = 1 ] && { log "refused: LG_TIER required"; exit 2; }
fi
above() { awk -v a="$1" -v b="$2" 'BEGIN{exit !(a>b)}'; }

# ---- admission -------------------------------------------------------------
# One admission per LG_SETTLE seconds, decided under a lock: two starters seeing the same 1m load must not both go.
settled() {  # settled L1
  local ok=1 every=${LG_SETTLE:-15}
  awk -v a="$1" -v b="${LG_FAST_1M:-44}" 'BEGIN{exit !(a<b)}' && every=$(awk -v a="$every" -v b="${LG_SETTLE_FAST:-5}" 'BEGIN{print (b<a)?b:a}')
  exec {lock_fd}>"$slot_dir/admit.lock"; flock "$lock_fd"
  if awk -v now="$(date +%s)" -v last="$(stat -c %Y "$slot_dir/last_admit" 2>/dev/null || echo 0)" -v every="$every" 'BEGIN{exit !(now-last>=every)}'; then
    touch "$slot_dir/last_admit"; ok=0
  fi
  exec {lock_fd}>&-
  return $ok
}

# The slot count is shared state (hysteresis needs the last value), kept in slot_dir/slots_now.
slot_count() {  # slot_count L1: prints the count
  local now; now=$(cat "$slot_dir/slots_now" 2>/dev/null)
  [ -n "$now" ] || now=$base  # another waiter's write may have truncated the file just now: empty broke `[ $busy -lt $slots ]`
  if above "$widen1" "$1"; then now=$high; elif ! above "$narrow1" "$1"; then now=$base; fi
  echo "$now" > "$slot_dir/slots_now.$$" && mv "$slot_dir/slots_now.$$" "$slot_dir/slots_now"  # temp + mv: a reader never sees it empty. The hysteresis state is the 3/5 count; the wide count is derived each time
  if awk -v kb="$(awk '/^MemAvailable:/{print $2}' "${LG_MEMINFO:-/proc/meminfo}")" -v gb="${LG_MEM_MIN_GB:-40}" 'BEGIN{exit !(kb>=gb*1024*1024)}'; then
    if above "$widest1" "$1"; then now=$widest; elif above "${LG_WIDE_1M:-50}" "$1"; then now=$wide; fi
  fi
  echo "$now" > "$slot_dir/slots_wide.$$" && mv "$slot_dir/slots_wide.$$" "$slot_dir/slots_wide"
  echo "$now"
}

# ---- order: one line "tier start pid start_line" per waiter in $prio, edited under the same lock discipline ----
prio_edit() {  # prio_edit add|del|first: first prints ok when no live waiter precedes this one
  # A TERM that lands inside this function runs the EXIT trap (prio_edit del) in the same shell: a second flock on a new descriptor
  # waits for the lock this shell already holds. The wait is bounded (LG_LOCK_WAIT, 2 s); a waiter's line that was not removed is
  # pruned by the next editor, which drops pids that are gone.
  local fd out=""; exec {fd}>"$slot_dir/prio.lock"
  flock -w "${LG_LOCK_WAIT:-2}" "$fd" || { exec {fd}>&-; return 1; }
  touch "$prio"
  # drop waiters whose process is gone
  # ... or is stopped (state T): a stopped head must not block the line. It re-adds itself with its old start when it resumes (first, below).
  local stopped=" $(ps -eo pid=,stat= 2>/dev/null | awk '$2 ~ /^T/ {printf "%s ", $1}')"
  while read -r t st pid q; do
    case "$stopped" in *" $pid "*) continue ;; esac
    kill -0 "$pid" 2>/dev/null && echo "$t $st $pid $q"
  done < "$prio" > "$prio.new"
  case $1 in
    add) my_start=$(date +%s.%N); echo "$tier $my_start $$ $start1" >> "$prio.new" ;;
    del) grep -v " $$ " "$prio.new" > "$prio.tmp"; mv "$prio.tmp" "$prio.new" ;;
    first) awk -v me="$$" '$3==me{f=1} END{exit !f}' "$prio.new" || echo "$tier $my_start $$ $start1" >> "$prio.new"  # an old copy's prune erases 4-field lines: re-add, same start
           out=$(awk '{printf "%d %s %d %s %s\n", ($1 >= 4) ? 1 : 0, $2, $1, $3, $4}' "$prio.new" | sort -k1,1n -k2,2n -k3,3n | awk -v me="$$" -v l1="$2" '{q=($5==""?25:$5)} q+0>l1+0{print ($4==me)?"ok":"no"; exit}') ;;  # tiers 1-3 by age (start stamp), tier breaks ties; tier 4 after all of them. A tier-2 job 2 h old waited behind a stream of fresh tier-1 probes: the old aging floors (t3 -> 2, t2 never -> 1) kept it behind forever
  esac
  mv "$prio.new" "$prio"; exec {fd}>&-
  [ "$1" != first ] || [ "$out" = ok ]
}

# Which slot files are in use by a job. The lock is the open file (/proc/locks names flock(1)'s own, already dead, pid), so the holders of slot.N are
# the processes with it open. A holder does not count when it is in state T (a SIGSTOPped or orphaned job; logged, never killed), when it is a waiter
# probing a free slot (its pid is in the prio file) or this shell; a child of another holder is the same job (the watchdog), and one pid holding two
# slot files counts once. A locked slot nobody is seen holding counts (the lock was taken after the scan). The old count was the number of locked
# files: a stopped cargo test held slot.3 for 28.8 h and a polling waiter's probe of a free slot read as a second holder.
declare -A slot_pids ppid_of is_stopped is_waiting counted
scan_holders() {
  local line slot pid
  slot_pids=(); ppid_of=(); is_stopped=(); is_waiting=(); counted=()
  while read -r slot pid; do slot_pids[$slot]="${slot_pids[$slot]:-} $pid"; done < <(find /proc -maxdepth 3 -path '/proc/[0-9]*/fd/*' -lname "$slot_dir/slot.*" -printf '%l %p\n' 2>/dev/null | awk '{ split($1, t, /\/slot\./); split($2, q, "/"); print t[2], q[3] }' | sort -u)
  while read -r pid ppid st; do ppid_of[$pid]=$ppid; case $st in T*) is_stopped[$pid]=1 ;; esac; done < <(ps -eo pid=,ppid=,stat= 2>/dev/null)
  [ -r "$prio" ] && while read -r _ _ pid _; do is_waiting[$pid]=1; done < "$prio"
}
# slot_busy N [note]: succeeds when slot N is held by a job that counts and was not counted through another slot file; the caller adds 1 per success.
slot_busy() {
  local pids=${slot_pids[$1]:-} p top
  [ -n "$pids" ] || return 0  # locked, holder unseen: counts
  for p in $pids; do
    [ "$p" = "$$" ] && continue
    [ -n "${is_waiting[$p]:-}" ] && continue
    if [ -n "${is_stopped[$p]:-}" ]; then [ "${2:-}" = note ] && log "not counting slot holder $p: stopped (state T)"; continue; fi
    top=$p; case " ${pids} " in *" ${ppid_of[$p]:-0} "*) top=${ppid_of[$p]} ;; esac
    if [ -z "${counted[$top]:-}" ]; then counted[$top]=1; return 0; fi
    # already counted through another slot file: the same job
  done
  return 1
}

admit() {
  mkdir -p "$slot_dir" || return 1
  [ "${LG_NO_WAIT:-}" = 1 ] || { prio_edit add; trap 'prio_edit del' EXIT; }
  local next_note=0 busy l1 l5 why slots
  while :; do
    read -r l1 _ < "$file"
    slots=$(slot_count "$l1")
    scan_holders; busy=0
    for i in $(seq 1 "$widest"); do  # slots beyond the current count stay held by running jobs
      exec {slot_fd}>"$slot_dir/slot.$i"
      if flock -n "$slot_fd"; then
        if [ "$busy" -lt "$slots" ] && above "$start1" "$l1" && { [ "${LG_NO_WAIT:-}" = 1 ] || prio_edit first "$l1"; } && settled "$l1"; then [ "${LG_NO_WAIT:-}" = 1 ] || prio_edit del; trap - EXIT; log "admit slot $i (1m $l1, slots $slots) tier $tier"; return 0; fi
        flock -u "$slot_fd"
      else
        slot_busy "$i" && busy=$((busy + 1))
      fi
      exec {slot_fd}>&-
    done
    [ "${LG_NO_WAIT:-}" = 1 ] && { log "no free slot or load too high"; return 75; }
    # a queued starter says so at once and every LG_WAIT_LOG seconds (60): "queued" must not look like "hung"
    if awk -v now="$(date +%s.%N)" -v due="$next_note" 'BEGIN{exit !(now>=due)}'; then
      read -r l1 l5 _ < "$file"
      why=load; [ "$busy" -ge "$slots" ] && why=slot
      for i in $(seq 1 "$widest"); do slot_busy "$i" note; done
      log "waiting for $why (1m $l1, 5m $l5, slots $busy/$slots busy)"
      next_note=$(awk -v now="$(date +%s.%N)" -v every="${LG_WAIT_LOG:-60}" 'BEGIN{printf "%.3f", now+every}')
    fi
    sleep "$poll"
  done
}
admit || exit $?

# ---- the process tree ------------------------------------------------------
tree_pgids() {  # tree_pgids ROOT: the process group of ROOT and of every descendant
  ps -eo pid=,ppid=,pgid= | awk -v root="$1" '
    { ppid[$1] = $2; pg[$1] = $3 }
    END { seen[root] = 1; n = 1; queue[1] = root; out[pg[root]] = 1
          for (i = 1; i <= n; i++) for (p in ppid) if (ppid[p] == queue[i] && !(p in seen)) { seen[p] = 1; queue[++n] = p; out[pg[p]] = 1 }
          for (g in out) print g }'
}
state=$(mktemp)
CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-${LG_CARGO_JOBS:-12}} LG_GUARDED=1 setsid "$@" {slot_fd}>&- & child=$!
stopped=0
jobs_dir=$slot_dir/jobs; mkdir -p "$jobs_dir"; echo "$$ $child $*" > "$jobs_dir/$$"  # the record --cancel reads; removed on exit
log "job $$ pgid $child: load_guard.sh --cancel $$ stops it"
signal_list() { for g in $(cat "$state"); do kill "-$1" -- "-$g" 2>/dev/null; done; }
stop_tree() {
  : > "$state"
  for _ in 1 2 3; do  # a child forked between the listing and the STOP is caught on the next pass
    for g in $(tree_pgids "$child"); do grep -qx "$g" "$state" || { echo "$g" >> "$state"; kill -STOP -- "-$g" 2>/dev/null; }; done
  done
}
resume_tree() { signal_list CONT; }
cleanup() { resume_tree; rm -f "$state" "$jobs_dir/$$"; }
trap 'cleanup; kill -TERM -- "-$child" 2>/dev/null; log "signalled: groups continued, command terminated"; exit 130' INT TERM
trap 'cleanup' EXIT
# watchdog: survives a SIGKILL of this wrapper and continues the recorded groups
setsid bash -c 'while kill -0 "$1" 2>/dev/null; do sleep 1; done; for g in $(cat "$2" 2>/dev/null); do kill -CONT -- "-$g" 2>/dev/null; done' _ "$$" "$state" >/dev/null 2>&1 &

while kill -0 "$child" 2>/dev/null; do
  read -r l1 l5 _ < "$file"
  if [ $stopped = 0 ] && { above "$l5" "$stop5" || above "$l1" "$stop1"; }; then
    stop_tree; stopped=1; log "stop groups $(tr '\n' ' ' < "$state")(1m $l1, 5m $l5)"
  elif [ $stopped = 1 ] && { above "$resume5" "$l5" || above "$resume1" "$l1"; }; then
    resume_tree; stopped=0; log "resume groups $(tr '\n' ' ' < "$state")(1m $l1, 5m $l5)"
  fi
  sleep "$poll"
done
wait "$child"; rc=$?
log "release (rc $rc)"
exit $rc
