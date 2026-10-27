# urgent_listed FILE N...: the PRs of N... that FILE lists (PR numbers, # comments), in FILE's order. The runner makes them a batch of their own, ahead of the rest.
urgent_listed() {
  local file=$1 listed n; shift
  listed=$(sed 's/#.*//' "$file" 2>/dev/null | tr -s ' \t' '\n' | grep -E '^[0-9]+$')
  for n in $listed; do printf '%s\n' "$@" | grep -qxF -- "$n" && echo "$n"; done | awk '!seen[$0]++'
}
