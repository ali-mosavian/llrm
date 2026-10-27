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
