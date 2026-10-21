# refusal_lines LOG: why a landing was refused: the `left out:` reasons (first three) and the REFUSED summary, none when the summary line says `REFUSED: -`.
refusal_lines() {
  { grep -q 'REFUSED: -$' "$1" || { grep 'left out:' "$1" | head -3; grep -o 'REFUSED.*' "$1" | head -2; }; } | tr '\n' ' ' | cut -c1-700
}
