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
# Regression (2026-11-08): cells built div_constants_* without its `// flags: -fno-inline-functions`, so a z80 cell read 145 kB
# for a 17.5 kB row and a retracted worse-cell report went to five owners. A cell compiles with its file's own flags
# (not the level, not machine flags) and says which in the row.
printf '// flags: -O2 -fno-inline-functions -m32 | -Os\nint main(void) { return 0; }\n' > "$d/p.c"
cat > "$d/fake-cc" <<'FAKE'
#!/bin/sh
echo "$@" >> "$ARGS"
for a in "$@"; do [ "$prev" = -o ] && out=$a; prev=$a; done
printf '\tnop\n' > "$out"
FAKE
for v in base new; do { cat "$d/fake-cc"; echo "# $v"; } > "$d/$v/llrm-cc"; chmod +x "$d/$v/llrm-cc"; done
export ARGS="$d/args"
echo "x t -O2 $d/p.c" > "$d/list"
out=$("$here/cells.sh" "$d" "$d/list" 2>&1)
case $out in *"flags: -fno-inline-functions") ;; *) echo "FAIL: stamp lacks the file's flags: $out"; exit 1;; esac
grep -q -- "-fno-inline-functions" "$d/args" || { echo "FAIL: cell compiled without the file's flags"; exit 1; }
if grep -q -- "-m32" "$d/args"; then echo "FAIL: machine flag taken from the file"; exit 1; fi
echo ok
