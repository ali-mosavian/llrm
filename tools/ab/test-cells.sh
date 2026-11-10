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
if [ -n "$FAKE_TEXT" ]; then printf '\tnop\n' > "$out"; else
  case " $* " in *" -c "*) printf 'int f(void){return 1;}' | cc -x c -c -o "$out" - ;; *) printf '\tnop\n' > "$out" ;; esac
fi
FAKE
for v in base new; do { cat "$d/fake-cc"; echo "# $v"; } > "$d/$v/llrm-cc"; chmod +x "$d/$v/llrm-cc"; done
export ARGS="$d/args"
# Regression (2026-11-10): size(1) failing on a z80/6502/OMF object made cells.sh print `stat -c %s` of the file as the code bytes
# (+1052 where the assembled _CODE was +175). A text "object" no reader parses is an EMPTY cell naming why, never a number.
echo "x t -O2 $d/p.c" > "$d/list"
out=$(FAKE_TEXT=1 "$here/cells.sh" "$d" "$d/list" 2>&1)
case $out in "EMPTY t -O2 $d/p.c"*) ;; *) echo "FAIL: unreadable object reported as: $out"; exit 1;; esac
case $out in *" bytes "[0-9]*) echo "FAIL: a size was printed for an unreadable object: $out"; exit 1;; esac
out=$("$here/cells.sh" "$d" "$d/list" 2>&1)
case $out in *"flags: -fno-inline-functions") ;; *) echo "FAIL: stamp lacks the file's flags: $out"; exit 1;; esac
grep -q -- "-fno-inline-functions" "$d/args" || { echo "FAIL: cell compiled without the file's flags"; exit 1; }
if grep -q -- "-m32" "$d/args"; then echo "FAIL: machine flag taken from the file"; exit 1; fi
# Regression (2026-11-10): CELLS_FLAGS was documented and never read, so a `CELLS_FLAGS=-fPIC` sweep compiled flagless and its
# "-fPIC identical to -fPIE" rows were meaningless. The flags reach both compiles and the row says so.
: > "$d/args"
out=$(CELLS_FLAGS=-fPIC "$here/cells.sh" "$d" "$d/list" 2>&1)
[ "$(grep -c -- '-fPIC' "$d/args")" -ge 4 ] || { echo "FAIL: CELLS_FLAGS=-fPIC did not reach the compiles (base and new, -c and -S): $(cat "$d/args")"; exit 1; }
case $out in *"env: -fPIC") ;; *) echo "FAIL: row does not state CELLS_FLAGS: $out"; exit 1;; esac
echo ok
