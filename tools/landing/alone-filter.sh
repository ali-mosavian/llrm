# alone_filter N...: the PRs that pass the alone check (parity_squash.py alone); every PR when the check itself cannot run, so a broken check never stalls the queue.
# ALONE_CMD is the command (default: python at nice 10; its cargo runs take their own buildslot, in parallel up to ALONE_JOBS, for at most ALONE_CAP seconds); its stderr is kept in alone.log.
alone_filter() {
  local kept
  if kept=$(${ALONE_CMD:-nice -n 10 python3 "${HERE:-.}/../parity_squash.py" alone} "$@" 2> >(tee -a "${LAND_TMP:-/tmp/landing}/alone.log" >&2)); then echo $(grep -xE '[0-9]+' <<< "$kept")
  else echo "alone check failed to run; the batch goes unfiltered" >&2; echo "$@"; fi
}

# alone_warm N...: the verdict cache filled for N... in the background while a batch squashes. ALONE_CMD (echo = precheck off) is respected, like alone_filter; the default runs at nice 19.
alone_warm() {
  ALONE_CAP=${ALONE_WARM_CAP:-3000} nohup ${ALONE_CMD:-nice -n 19 python3 "${HERE:-.}/../parity_squash.py" alone} "$@" > /dev/null 2>> "${LAND_TMP:-/tmp/landing}/alone.log" &
}

# alone_warm_at_squash LOG N...: alone_warm once LOG shows the batch's squash stage, never while its lib-tests run (the next batch's precheck beside them took 6 of 8 cargo processes at load 108);
# gives up when LOG.done appears (the batch ended without squashing).
alone_warm_at_squash() {
  local log=$1; shift
  ( until grep -q 'STAGE squash' "$log" 2>/dev/null; do [ -e "$log.done" ] && exit 0; sleep "${ALONE_WARM_POLL:-5}"; done; alone_warm "$@" ) > /dev/null 2>&1 &
}
