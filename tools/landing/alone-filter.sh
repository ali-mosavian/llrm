# alone_filter N...: the PRs that pass the alone check (parity_squash.py alone). Nothing to filter is not a run (the check exits 2 on no PR: 55 "failed to run" lines were mostly that). A check that fails
# is run once more, then the cycle is held (nothing printed, so no batch): a filter that fails open lets an unchecked PR into a batch (#5008 broke the 21:53 batch).
# ALONE_CMD is the command (default: python at nice 10; its cargo runs take their own buildslot, in parallel up to ALONE_JOBS, for at most ALONE_CAP seconds); its stderr is kept in alone.log.
alone_filter() {
  [ $# -eq 0 ] && return 0
  [ "${LAND_SKIP_ALONE:-}" = 1 ] && { echo "$@"; return 0; }  # the user's bulk order: the batch's own halving finds the bad PRs
  local kept try rc
  for try in 1 2; do
    kept=$(${ALONE_CMD:-nice -n 10 python3 "${HERE:-.}/../parity_squash.py" alone} "$@" 2> >(tee -a "${LAND_TMP:-/tmp/landing}/alone.log" >&2)); rc=$?
    if [ $rc -eq 0 ]; then echo $(grep -xE '[0-9]+' <<< "$kept"); return 0; fi
    echo "alone check failed (exit $rc, try $try of 2): ${kept:0:200}" >&2
    [ $try -lt 2 ] && sleep "${ALONE_RETRY_WAIT:-5}"
  done
  echo "alone check failed twice: holding the cycle, no unfiltered batch" >&2
}

# alone_warm N...: (ends after ALONE_WARM_TIMEOUT, default 3600 s) the verdict cache filled for N... in the background while a batch squashes. ALONE_CMD (echo = precheck off) is respected, like alone_filter; the default runs at nice 19.
alone_warm() {
  [ "${LAND_SKIP_ALONE:-}" = 1 ] && return 0
  ALONE_CAP=${ALONE_WARM_CAP:-3000} nohup timeout "${ALONE_WARM_TIMEOUT:-3600}" ${ALONE_CMD:-nice -n 19 python3 "${HERE:-.}/../parity_squash.py" alone} "$@" > /dev/null 2>> "${LAND_TMP:-/tmp/landing}/alone.log" &
}

# alone_warm_at_squash LOG N...: alone_warm once LOG shows the batch's squash stage, never while its lib-tests run (the next batch's precheck beside them took 6 of 8 cargo processes at load 108);
# gives up when LOG.done appears (the batch ended without squashing).
alone_warm_at_squash() {
  local log=$1; shift
  ( until grep -q 'STAGE squash' "$log" 2>/dev/null; do [ -e "$log.done" ] && exit 0; sleep "${ALONE_WARM_POLL:-5}"; done; alone_warm "$@" ) > /dev/null 2>&1 &
}
