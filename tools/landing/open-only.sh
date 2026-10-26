# open_only N...: prints the PRs that are still OPEN; says `N: SKIPPED not open` on stderr for the rest
open_only() { for m; do [ "$(gh pr view "$m" --repo "${R:-x}" --json state -q .state)" = OPEN ] && echo "$m" || echo "$m: SKIPPED not open" >&2; done; }

# pop_open Q N...: prints the open PRs of N... and removes only those from the queue file Q; a PR that is not open or never entered a batch keeps its line (the rebuilt queue drops the closed).
pop_open() { local q=$1 m; shift; for m in $(open_only "$@" 2>/dev/null); do sed -i "/^$m\$/d" "$q"; echo "$m"; done; }
