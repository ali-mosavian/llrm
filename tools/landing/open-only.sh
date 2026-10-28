# open_only N...: prints the PRs that are still OPEN and not draft; says `N: SKIPPED not open|draft` on stderr for the rest
source "$(dirname "${BASH_SOURCE[0]}")/pr-state.sh"
open_only() {
  local s d line
  for m; do
    [[ $m =~ ^[0-9]+$ ]] || { echo "$m: SKIPPED not a PR number" >&2; continue; }
    s=$(cached_state "$m" state); d=$(cached_state "$m" isDraft)
    [ -n "$s" ] || { line=$(gh pr view "$m" --repo "${R:-x}" --json state,isDraft -q '.state+" "+(.isDraft|tostring)'); s=${line%% *}; d=${line#* }; }
    [ "$s" = OPEN ] || { echo "$m: SKIPPED not open" >&2; continue; }
    [ "$d" = true ] && echo "$m: SKIPPED draft (drafted after the queue was built)" >&2 || echo "$m"
  done
}

# pop_open Q N...: prints the open PRs of N... and removes only those from the queue file Q; a PR that is not open or never entered a batch keeps its line (the rebuilt queue drops the closed).
pop_open() { local q=$1 m; shift; for m in $(open_only "$@" 2>/dev/null); do sed -i "/^$m\$/d" "$q"; echo "$m"; done; }
