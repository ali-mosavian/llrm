# last_refusals LOG PR...: one line per PR, the last `PR: REFUSED` line of LOG (`PR: none`); the log repeats each refusal every tick
last_refusals() {
  local log=$1 m line; shift
  for m; do
    line=$(grep -E "^$m: REFUSED" "$log" | tail -1)
    echo "${line:-$m: none}" | cut -c1-300
  done
}
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  log=${1:?usage: last-refusals.sh LOG [PR...]}; shift
  [ $# -gt 0 ] || set -- $(gh pr list --repo "${R:?set R=owner/repo or name PRs}" --state open --json number,isDraft -q '.[]|select(.isDraft|not)|.number' | sort -n)
  last_refusals "$log" "$@"
fi
