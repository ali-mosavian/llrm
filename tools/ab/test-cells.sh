#!/usr/bin/env bash
# Regression: a CGP A/B (2026-11-05) compared dists built from different commits and another
# read both columns from one unset array slot, so "no change" and "changed" were both wrong.
# cells.sh must refuse identical binaries and must report distinct base/new columns.
here=$(cd "$(dirname "$0")" && pwd); d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
mkdir -p "$d/base" "$d/new"
for b in llrm-cc llrm-llc; do printf '#!/bin/sh\necho same\n' > "$d/base/$b"; cp "$d/base/$b" "$d/new/$b"; chmod +x "$d/base/$b" "$d/new/$b"; done
echo "x t -O2 f.c" > "$d/list"
"$here/cells.sh" "$d" "$d/list" >/dev/null 2>&1 && { echo "FAIL: identical binaries accepted"; exit 1; }
# distinct binaries, compiler produces nothing: the cell is EMPTY, never a number
printf '#!/bin/sh\necho other\n' > "$d/new/llrm-cc"; printf '#!/bin/sh\necho other\n' > "$d/new/llrm-llc"
out=$("$here/cells.sh" "$d" "$d/list" 2>&1)
case $out in EMPTY*) ;; *) echo "FAIL: empty cell reported as $out"; exit 1;; esac
echo ok
