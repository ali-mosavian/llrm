source "$(dirname "${BASH_SOURCE[0]}")/pr-state.sh"
# aged N...: the PRs whose last commit is at least HEAD_MIN_AGE seconds old (default 900); says `N: waits, head younger than 15 min` on stderr for the rest.
# A head pushed a minute ago is a fix in progress (#4713's hand merge was pushed at 14:16 and fixed at 14:18, inside the batch that refused it).
aged() {
  local now m committed
  now=$(date +%s)
  for m; do
    committed=$(cached_state "$m" committedDate); [ -n "$committed" ] || committed=$(gh pr view "$m" --repo "${R:-x}" --json commits -q '.commits[-1].committedDate')
    committed=$(date -d "$committed" +%s 2>/dev/null) || committed=0
    if [ $((now - committed)) -ge "${HEAD_MIN_AGE:-900}" ]; then echo "$m"; else echo "$m: waits, head younger than 15 min" >&2; fi
  done
}
