# open_only N...: prints the PRs that are still OPEN; says `N: SKIPPED not open` on stderr for the rest
open_only() { for m; do [ "$(gh pr view "$m" --repo "${R:-x}" --json state -q .state)" = OPEN ] && echo "$m" || echo "$m: SKIPPED not open" >&2; done; }
