#!/usr/bin/env bash
# Regression (2026-11-13): vs.sh stopped at the first compile that failed on our side and never ran the pin, and ignored `// targets:`:
# 160 wasm32 cells were reported as failures, 152 of them programs the corpus declares not for wasm32. vs.sh must run both sides
# and print n/a for a declared program. Run from the repo root.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d); trap 'rm -rf "$d"' EXIT; mkdir -p "$d/dist"
printf '#!/bin/sh\nexit 1\n' > "$d/dist/llrm-cc"; chmod +x "$d/dist/llrm-cc"
printf '// targets: dos\nint main(void){return 0;}\n' > "$d/dos.c"
printf 'int main(void){return 0;}\n' > "$d/ok.c"
out=$(bash "$here/vs.sh" "$d/dist" O2 "$d/dos.c" "$d/ok.c")
echo "$out" | grep -q '^dos.c -O2 n/a' || { echo "FAIL: a DOS-only program is not n/a: $out"; exit 1; }
echo "$out" | grep -qE '^ok.c -O2 ours FAILED.* pin [0-9]+$' || { echo "FAIL: the pin was not run after ours failed: $out"; exit 1; }
echo ok
