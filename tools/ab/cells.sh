#!/usr/bin/env bash
# cells.sh DIR LIST: per-cell code bytes, data bytes and instruction count, DIR/base vs DIR/new.
# LIST lines: "<anything> <target> <-Olevel> <file.c>". Refuses to run when base and new are
# the same binary (a sweep of a build against itself reads "no change" everywhere).
# Env: CELLS_FLAGS (extra cc flags, passed to every compile), CELLS_INC (include flags). Each row ends `flags: WORDS`: the file's own flags, `-` for none,
# then ` env: WORDS` when CELLS_FLAGS is set. CELLS_FLAGS was documented and never read: a `-fPIC` sweep (2026-11-10) compiled flagless.
set -u
. "$(dirname "$0")/../ab_env.sh"; ab_env_check || exit 2
D=$1; L=$2; repo=$(cd "$(dirname "$0")/../.." && pwd)
for b in llrm-cc llrm-llc; do
  [ -f "$D/base/$b" ] && [ -f "$D/new/$b" ] || { echo "cells: missing $b in base or new" >&2; exit 2; }
  [ "$(md5sum < "$D/base/$b")" != "$(md5sum < "$D/new/$b")" ] || { echo "cells: base and new $b are the same binary" >&2; exit 2; }
done
[ "$(readlink -f "$D/base")" != "$(readlink -f "$D/new")" ] || { echo "cells: base and new are one directory" >&2; exit 2; }
. "$(dirname "$0")/../fresh_compile.sh"
# An include dir that is not there is a broken sweep, not a program that fails (2026-11-10: four jpeg/stft programs came out EMPTY at -m32 for lack of
# `-I toolchain/clang/os/x86-m32` and were first counted as programs that do not compile).
prev=""
for word in ${CELLS_INC:-}; do
  dir=""; case $prev in -I) dir=$word;; *) case $word in -I?*) dir=${word#-I};; esac;; esac
  prev=$word
  [ -z "$dir" ] || [ -d "$dir" ] || { echo "cells: include dir '$dir' (CELLS_INC) does not exist" >&2; exit 2; }
done
w=$(mktemp -d); trap 'rm -rf "$w"' EXIT
declare -A text dat ins why classes
empty=0; na=0; compiled=0
# First line of a compile's stderr that says what failed; its class counts in the summary.
first_error() { grep -m1 -iE 'error|not found' "$1" 2>/dev/null || head -n1 "$1" 2>/dev/null; }
class_of() { case $1 in *"file not found"*) echo "missing include";; *rror*) echo "compile error";; "") echo "no output";; *) echo "other";; esac; }
while read -r _ tg lvl f; do
  # A program that is not for this target (`// targets:` tag or the default tags, e.g. DOS-only) is no cell: n/a, counted, never EMPTY.
  if reason=$(python3 "$(dirname "$0")/../corpus_targets.py" check "$tg" "$f" 2>/dev/null); then :; elif [ $? -eq 3 ]; then
    echo "N/A $tg $lvl $f ($reason)"; na=$((na + 1)); continue
  fi
  fl=""; case $tg in x86_16-wat) fl=-march=i386;; x86_32-linux) fl=-m32;; esac
  # The program's own flags (`// flags:`, first variant): a cell built without them is another program (div_constants_*
  # without -fno-inline-functions inlined to 145 kB on z80 where the row is 17.5 kB).
  pf=$(python3 "$(dirname "$0")/cell_flags.py" "$f")
  # A program that includes llrm_os.h or bench_io.h needs the posix runtime dir (and bench/): a sweep without it refused every one at every target
  # ('llrm_os.h' file not found, 52 cells) and hid the real errors behind it. corpus_refusals.py gives the same two dirs. The posix dir goes after every other: a target's own llrm_os.h (x86-m16's DOS one has LLRM_OS_TEXT_SCREEN) must win.
  os_inc=""; grep -qE '#[[:space:]]*include[[:space:]]*"[^"]*(llrm_os|bench_io)\.h"' "$f" 2>/dev/null && os_inc="-idirafter $repo/runtime/shared/posix -I $repo/bench"
  for v in base new; do
    LLRM_CC_TARGET=$tg fresh_compile "$w/$v.o" "$D/$v/llrm-cc" $fl $pf $lvl ${CELLS_FLAGS:-} ${CELLS_INC:-} $os_inc -c "$f" -o "$w/$v.o" >/dev/null 2>"$w/$v.err" || { why[$v]=$(first_error "$w/$v.err"); continue; }
    # wasm has no text output (`-S` writes the binary): its instructions are counted from the object's disassembly.
    case $tg in wasm*) cp "$w/$v.o" "$w/$v.s";; *)
      LLRM_CC_TARGET=$tg fresh_compile "$w/$v.s" "$D/$v/llrm-cc" $fl $pf $lvl ${CELLS_FLAGS:-} ${CELLS_INC:-} $os_inc -S "$f" -o "$w/$v.s" >/dev/null 2>"$w/$v.err" || { why[$v]=$(first_error "$w/$v.err"); continue; };; esac
    [ -s "$w/$v.o" ] && [ -s "$w/$v.s" ] || continue
    # Code bytes of the object (object_bytes.py: per target; z80/6502 assemble their .s). Never the file's size: a failure is an EMPTY cell.
    read -r text[$v] dat[$v] _ < <(uv run -q --project "$(dirname "$0")/.." python "$(dirname "$0")/object_bytes.py" "$tg" "$w/$v.o" 2>>"$w/err") || { unset "text[$v]" "dat[$v]"; }
    case $tg in wasm*) ins[$v]=$(wasm-objdump -d "$w/$v.o" 2>/dev/null | grep -cE '^ [0-9a-f]+: ');;
      *) ins[$v]=$(grep -cE '^[[:space:]]+[a-z][a-z0-9.]*([[:space:]]|$)' "$w/$v.s");; esac
  done
  if [ -z "${text[base]:-}" ] || [ -z "${text[new]:-}" ] || [ "${ins[base]:-0}" -eq 0 ] || [ "${ins[new]:-0}" -eq 0 ]; then
    reason=${why[base]:-${why[new]:-}}; [ -n "$reason" ] || reason=$(head -1 "$w/err" 2>/dev/null)
    echo "EMPTY $tg $lvl $f$( [ -n "$reason" ] && printf ' (%s)' "$reason" )" ; : > "$w/err"; empty=$((empty + 1))
    c=$(class_of "$reason"); classes[$c]=$(( ${classes[$c]:-0} + 1 ))
    unset 'text[base]' 'text[new]' 'dat[base]' 'dat[new]' 'ins[base]' 'ins[new]' 'why[base]' 'why[new]'; continue
  fi
  compiled=$((compiled + 1))
  echo "$tg $lvl $(basename "$f") bytes ${text[base]} ${text[new]} data ${dat[base]} ${dat[new]} instrs ${ins[base]} ${ins[new]} flags: ${pf:--}${CELLS_FLAGS:+ env: $CELLS_FLAGS}"
  unset 'text[base]' 'text[new]' 'dat[base]' 'dat[new]' 'ins[base]' 'ins[new]'
done < "$L"
by=""; for c in "${!classes[@]}"; do by="${by:+$by, }$c: ${classes[$c]}"; done
echo "cells: compiled=$compiled n/a=$na EMPTY=$empty${by:+ ($by)}" >&2
# A cell that built nothing is a failed measurement, not a quiet one.
[ "$empty" -eq 0 ] || { echo "cells: $empty EMPTY cell(s)" >&2; exit 1; }
