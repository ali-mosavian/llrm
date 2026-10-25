# alone_filter N...: the PRs that pass the alone check (parity_squash.py alone); every PR when the check itself cannot run, so a broken check never stalls the queue.
# ALONE_CMD is the command (default: python at nice 10; its cargo runs take their own buildslot, in parallel up to ALONE_JOBS, for at most ALONE_CAP seconds); its stderr is kept in alone.log.
alone_filter() {
  local kept
  if kept=$(${ALONE_CMD:-nice -n 10 python3 "${HERE:-.}/../parity_squash.py" alone} "$@" 2> >(tee -a "${LAND_TMP:-/tmp/landing}/alone.log" >&2)); then echo $kept
  else echo "alone check failed to run; the batch goes unfiltered" >&2; echo "$@"; fi
}
