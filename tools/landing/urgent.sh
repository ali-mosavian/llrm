# urgent_listed FILE N...: the PRs of N... that FILE lists (PR numbers, # comments), in FILE's order. The runner makes them a batch of their own, ahead of the rest.
urgent_listed() {
  local file=$1 listed n; shift
  listed=$(sed 's/#.*//' "$file" 2>/dev/null | tr -s ' \t' '\n' | grep -E '^[0-9]+$')
  for n in $listed; do printf '%s\n' "$@" | grep -qxF -- "$n" && echo "$n"; done | awk '!seen[$0]++'
}

# urgent_from_queue FILE QUEUE: urgent_listed over the WHOLE queue file, not the batch window: #5558 and #5561 (the fixes for a red parity tip) sat at lines 59 and 62 of a window of 40+24 and
# the 22:06 batch of 32 left them out.
urgent_from_queue() { urgent_listed "$1" $(cat "$2" 2>/dev/null); }
