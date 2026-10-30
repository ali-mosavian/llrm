# stale_batch_lines: `STALE BATCH pid N: M min old, heads moved since it began: #a #b` for each merge-batch process older than STALE_BATCH_SECS (default 1200) with a member whose last commit is newer than the process start (it read the head before that).
# STALE_PS and STALE_COMMITTED (PR -> epoch) are the seams for the test.
pr_committed() { date -d "$(gh pr view "$1" --repo "${R:-ali-mosavian/llrm}" --json commits -q '.commits[-1].committedDate')" +%s 2>/dev/null; }
stale_batch_lines() {
  local now; now=$(date +%s)
  ${STALE_PS:-ps -eo pid=,etimes=,args=} | grep 'parity_squash.py merge-batch' | grep -v -e grep -e ' uv run ' | while read -r pid secs args; do
    [ "$secs" -gt "${STALE_BATCH_SECS:-1200}" ] || continue
    local moved="" n committed
    for n in $(grep -oE '[0-9]+' <<< "${args#*merge-batch}"); do
      committed=$(${STALE_COMMITTED:-pr_committed} "$n"); [ -n "$committed" ] && [ "$committed" -gt $((now - secs)) ] && moved+=" #$n"
    done
    [ -n "$moved" ] && echo "STALE BATCH pid $pid: $((secs / 60)) min old, heads moved since it began:$moved"
  done
}
# stale_batch_watch LOG: every STALE_POLL seconds (60) until LOG.done appears, in the background; its lines go to the runner's own log, which a batch blocks the main loop from writing to.
stale_batch_watch() { ( until [ -e "$1.done" ]; do stale_batch_lines; sleep "${STALE_POLL:-60}"; done ) & }
