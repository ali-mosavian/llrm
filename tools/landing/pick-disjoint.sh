# pick_disjoint MAX N...: the first MAX of the PRs, in the order given, whose changed files are disjoint from those already picked.
# Mates then cannot conflict. Generated pages the gate regenerates (docs/optimizations/**/readme.md) do not count.
IGNORE_FILES=${IGNORE_FILES:-'^docs/optimizations/.*readme\.md$'}
pick_disjoint() {
  local max=$1 used="" picked=0 n files; shift
  for n in "$@"; do
    [ "$picked" -ge "$max" ] && break
    files=$(gh pr view "$n" --repo "${R:-x}" --json files -q '.files[].path' | grep -vE "$IGNORE_FILES")
    if [ -n "$files" ] && echo "$files" | grep -qxFf <(echo "$used") 2>/dev/null; then continue; fi
    used+=$'\n'"$files"; picked=$((picked+1)); echo "$n"
  done
}
