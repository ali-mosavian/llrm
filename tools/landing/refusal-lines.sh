# refusal_lines LOG: why a landing was refused: the `left out:` reasons (first three) and the REFUSED summary, none when the summary line says `REFUSED: -`.
refusal_lines() {
  { grep -q 'REFUSED: -$' "$1" || { grep 'left out:' "$1" | head -3; grep -o 'REFUSED.*' "$1" | head -2; }; } | tr '\n' ' ' | cut -c1-700
}
# batch_refusal_reason LOG PR: why PR was not merged by the batch whose log is LOG: its own `#PR: REFUSED` line, else the first two lines naming it or REFUSED, else (a crashed batch prints neither) the last two log lines.
batch_refusal_reason() {
  local why
  why=$(grep -E "#$2: REFUSED" "$1" | head -1)
  [ -n "$why" ] || why=$(grep -E "#$2|REFUSED" "$1" | head -2 | tr '\n' ' ')
  [ -n "$why" ] || why="no REFUSED line; log tail: $(grep -v '^[[:space:]]*$' "$1" | tail -2 | tr '\n' ' ')"
  echo "$why" | cut -c1-400
}

# landed_state LOG PR: the PR's state, waiting for GitHub to flip it to MERGED when the batch pushed it (PUSHED: line naming it): the label read REFUSED for the two PRs of each push (02:47, 02:51) because the flip lags the push by seconds.
landed_state() { local s i; for i in 1 2 3 4 5 6 7 8 9 10 11 12; do s=$(gh pr view "$2" --repo "${R:-x}" --json state -q .state); [ "$s" = MERGED ] && break; grep -qE "PUSHED: .*#$2( |\$)" "$1" || break; sleep "${PUSH_SETTLE:-5}"; done; echo "$s"; }
