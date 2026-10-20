# refusal_lines LOG: the first two REFUSED reasons of a landing log, none when the summary line says `REFUSED: -`.
refusal_lines() {
  grep -o 'REFUSED.*' "$1" | grep -v '^REFUSED: -$' | head -2 | tr '\n' ' ' | cut -c1-500
}
