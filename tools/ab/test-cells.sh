#!/usr/bin/env bash
# Regression: a CGP A/B (2026-11-05) compared dists built from different commits and another
# read both columns from one unset array slot, so "no change" and "changed" were both wrong.
# cells.sh must refuse identical binaries and must report distinct base/new columns.
here=$(cd "$(dirname "$0")" && pwd); cells=${CELLS_SH:-$here/cells.sh}; d=$(mktemp -d); trap 'rm -rf "$d"' EXIT
mkdir -p "$d/base" "$d/new"
for b in llrm-cc llrm-llc; do printf '#!/bin/sh\necho same\n' > "$d/base/$b"; cp "$d/base/$b" "$d/new/$b"; chmod +x "$d/base/$b" "$d/new/$b"; done
echo "x t -O2 f.c" > "$d/list"
"$cells" "$d" "$d/list" >/dev/null 2>&1 && { echo "FAIL: identical binaries accepted"; exit 1; }
# distinct binaries, compiler produces nothing: the cell is EMPTY, never a number
printf '#!/bin/sh\necho other\n' > "$d/new/llrm-cc"; printf '#!/bin/sh\necho other\n' > "$d/new/llrm-llc"
out=$("$cells" "$d" "$d/list" 2>&1)
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
out=$(FAKE_TEXT=1 "$cells" "$d" "$d/list" 2>&1)
case $out in "EMPTY t -O2 $d/p.c"*) ;; *) echo "FAIL: unreadable object reported as: $out"; exit 1;; esac
case $out in *" bytes "[0-9]*) echo "FAIL: a size was printed for an unreadable object: $out"; exit 1;; esac
out=$("$cells" "$d" "$d/list" 2>/dev/null)
case $out in *"flags: -fno-inline-functions") ;; *) echo "FAIL: stamp lacks the file's flags: $out"; exit 1;; esac
grep -q -- "-fno-inline-functions" "$d/args" || { echo "FAIL: cell compiled without the file's flags"; exit 1; }
if grep -q -- "-m32" "$d/args"; then echo "FAIL: machine flag taken from the file"; exit 1; fi
# Regression (2026-11-10): CELLS_FLAGS was documented and never read, so a `CELLS_FLAGS=-fPIC` sweep compiled flagless and its
# "-fPIC identical to -fPIE" rows were meaningless. The flags reach both compiles and the row says so.
: > "$d/args"
out=$(CELLS_FLAGS=-fPIC "$cells" "$d" "$d/list" 2>/dev/null)
[ "$(grep -c -- '-fPIC' "$d/args")" -ge 4 ] || { echo "FAIL: CELLS_FLAGS=-fPIC did not reach the compiles (base and new, -c and -S): $(cat "$d/args")"; exit 1; }
case $out in *"env: -fPIC") ;; *) echo "FAIL: row does not state CELLS_FLAGS: $out"; exit 1;; esac
# Regression (2026-11-10): cells.sh had no tag skip and said nothing about why a cell was EMPTY. A DOS-only source (`// targets: dos`, `far`
# pointers) at -m32 printed EMPTY, was counted as a program that does not compile, and four jpeg/stft cells missing `-I toolchain/clang/os/x86-m32`
# were first explained the same way. A program not for the target is `N/A` with its reason and a count; an EMPTY row names the first compiler
# error and the summary counts the rows by reason; an include dir that is not there stops the sweep (exit 2).
printf '// targets: dos\nshort far *p;\n' > "$d/dos.c"
echo "x x86_32-linux -Os $d/dos.c" > "$d/list"
out=$("$cells" "$d" "$d/list" 2>&1); rc=$?
case $out in *"N/A x86_32-linux -Os $d/dos.c (n/a, DOS-only)"*) ;; *) echo "FAIL: a DOS-only source at x86_32-linux is not N/A: $out"; exit 1;; esac
case $out in *EMPTY\ x86_32*) echo "FAIL: an n/a program printed as EMPTY: $out"; exit 1;; esac
case $out in *"n/a=1"*) ;; *) echo "FAIL: summary lacks the n/a count: $out"; exit 1;; esac
[ "$rc" -eq 0 ] || { echo "FAIL: an n/a cell failed the sweep (rc $rc)"; exit 1; }
cat > "$d/err-cc" <<'FAKE'
#!/bin/sh
echo "p.c:1:10: fatal error: 'llrm_os.h' file not found" >&2
exit 1
FAKE
for v in base new; do { cat "$d/err-cc"; echo "# $v"; } > "$d/$v/llrm-cc"; chmod +x "$d/$v/llrm-cc"; done
printf '#include "llrm_os.h"\nint main(void) { return 0; }\n' > "$d/need.c"
echo "x x86_32-linux -Os $d/need.c" > "$d/list"
out=$("$cells" "$d" "$d/list" 2>&1); rc=$?
case $out in *"EMPTY x86_32-linux -Os $d/need.c ("*"'llrm_os.h' file not found)"*) ;; *) echo "FAIL: EMPTY row lacks the first compiler error: $out"; exit 1;; esac
case $out in *"EMPTY=1 (missing include: 1)"*) ;; *) echo "FAIL: summary lacks the count by reason: $out"; exit 1;; esac
[ "$rc" -ne 0 ] || { echo "FAIL: a program that needs an include and had none passed the sweep"; exit 1; }
out=$(CELLS_INC="-I $d/absent" "$cells" "$d" "$d/list" 2>&1); rc=$?
[ "$rc" -eq 2 ] || { echo "FAIL: an absent include dir gave rc $rc: $out"; exit 1; }
case $out in *"include dir"*"absent"*) ;; *) echo "FAIL: absent include dir not named: $out"; exit 1;; esac
# Regression (2026-11-12): wasm32 `-S` writes the binary, so the text-line instruction count was 0 and every wasm cell read EMPTY. A wasm cell
# is counted from the object's disassembly and prints its bytes.
cat > "$d/m.wat" <<'WAT'
(module (func (export "f") (result i32) i32.const 1 i32.const 2 i32.add))
WAT
wat2wasm "$d/m.wat" -o "$d/m.wasm"
for v in base new; do printf '#!/bin/sh\nfor a in "$@"; do [ "$p" = -o ] && out=$a; p=$a; done\ncp %s "$out"\n# %s\n' "$d/m.wasm" "$v" > "$d/$v/llrm-cc"; chmod +x "$d/$v/llrm-cc"; done
echo "x wasm32 -O2 $d/p.c" > "$d/list"
out=$("$cells" "$d" "$d/list" 2>&1)
case $out in "wasm32 -O2 p.c bytes "[0-9]*" instrs 4 4"*) ;; *) echo "FAIL: a wasm cell is not counted from its object: $out"; exit 1;; esac
echo ok
# Regression (2026-11-13): a sweep with no CELLS_INC refused every program that includes llrm_os.h or bench_io.h at every target ('llrm_os.h' file not
# found, 52 cells, the real errors behind it). cells.sh gives such a source the posix runtime dir and bench/ itself; a source that does not include them
# gets neither.
for v in base new; do { cat "$d/fake-cc"; echo "# $v"; } > "$d/$v/llrm-cc"; chmod +x "$d/$v/llrm-cc"; done
printf '#include "../../../bench/bench_io.h"\nint main(void) { return 0; }\n' > "$d/io.c"
echo "x t -O2 $d/io.c" > "$d/list"
: > "$d/args"; unset CELLS_INC
"$cells" "$d" "$d/list" >/dev/null 2>&1
grep -q -- "runtime/shared/posix" "$d/args" || { echo "FAIL: a bench_io.h source compiled without the posix runtime dir: $(cat "$d/args")"; exit 1; }
# The posix dir must not shadow a target's own llrm_os.h (2026-11-13: -I put it first, and 16 os_screen/os_execute cells at x86_16-wat lost the DOS header's
# LLRM_OS_TEXT_SCREEN): it goes with -idirafter, never -I.
if grep -q -- "-I [^ ]*runtime/shared/posix" "$d/args"; then echo "FAIL: the posix runtime dir was given with -I, ahead of the target's own headers: $(cat "$d/args")"; exit 1; fi
grep -q -- "-idirafter [^ ]*runtime/shared/posix" "$d/args" || { echo "FAIL: the posix runtime dir was not given with -idirafter: $(cat "$d/args")"; exit 1; }
printf 'int main(void) { return 0; }\n' > "$d/plain.c"
echo "x t -O2 $d/plain.c" > "$d/list"
: > "$d/args"
"$cells" "$d" "$d/list" >/dev/null 2>&1
if grep -q -- "runtime/shared/posix" "$d/args"; then echo "FAIL: a source with no OS header got the posix runtime dir"; exit 1; fi

