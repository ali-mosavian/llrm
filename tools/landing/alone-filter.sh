# alone_filter N...: the PRs that pass the alone check (parity_squash.py alone); every PR when the check itself cannot run, so a broken check never stalls the queue.
# ALONE_CMD is the command (default: under buildslot); its stderr is kept in alone.log.
alone_filter() {
  local kept
  if kept=$(${ALONE_CMD:-nice -n 10 buildslot python3 "${HERE:-.}/../parity_squash.py" alone} "$@" 2> >(tee -a "${LAND_TMP:-/tmp/landing}/alone.log" >&2)); then echo $kept
  else echo "alone check failed to run; the batch goes unfiltered" >&2; echo "$@"; fi
}
