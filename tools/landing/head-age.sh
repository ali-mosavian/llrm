# The age is read on every call: $LAND_TMP/head-age.txt (seconds) wins over HEAD_MIN_AGE, so it changes without a restart.
# aged N...: the PRs whose last commit is at least HEAD_MIN_AGE seconds old (default 900); says `N: waits, head younger than 15 min` on stderr for the rest.
# A head pushed a minute ago is a fix in progress (#4713's hand merge was pushed at 14:16 and fixed at 14:18, inside the batch that refused it).
aged() {
  local now m committed min
  now=$(date +%s)
  min=$(cat "${LAND_TMP:-/tmp/landing}/head-age.txt" 2>/dev/null); [[ $min =~ ^[0-9]+$ ]] || min=${HEAD_MIN_AGE:-900}
  for m; do
    committed=$(date -d "$(gh pr view "$m" --repo "${R:-x}" --json commits -q '.commits[-1].committedDate')" +%s 2>/dev/null) || committed=0
    if [ $((now - committed)) -ge "$min" ]; then echo "$m"; else echo "$m: waits, head younger than $((min / 60)) min" >&2; fi
  done
}
