#!/usr/bin/env bash
# cells.sh DIR LIST: per-cell .text bytes and instruction count, DIR/base vs DIR/new.
# LIST lines: "<anything> <target> <-Olevel> <file.c>". Refuses to run when base and new are
# the same binary (a sweep of a build against itself reads "no change" everywhere).
# Env: CELLS_FLAGS (extra cc flags), CELLS_INC (include flags).
set -u
. "$(dirname "$0")/../ab_env.sh"; ab_env_check || exit 2
D=$1; L=$2
for b in llrm-cc llrm-llc; do
  [ -f "$D/base/$b" ] && [ -f "$D/new/$b" ] || { echo "cells: missing $b in base or new" >&2; exit 2; }
  [ "$(md5sum < "$D/base/$b")" != "$(md5sum < "$D/new/$b")" ] || { echo "cells: base and new $b are the same binary" >&2; exit 2; }
done
[ "$(readlink -f "$D/base")" != "$(readlink -f "$D/new")" ] || { echo "cells: base and new are one directory" >&2; exit 2; }
. "$(dirname "$0")/../fresh_compile.sh"
w=$(mktemp -d); trap 'rm -rf "$w"' EXIT
declare -A text ins
empty=0
while read -r _ tg lvl f; do
  fl=""; case $tg in x86_16-wat) fl=-march=i386;; x86_32-linux) fl=-m32;; esac
  for v in base new; do
    LLRM_CC_TARGET=$tg fresh_compile "$w/$v.o" "$D/$v/llrm-cc" $fl $lvl ${CELLS_INC:-} -c "$f" -o "$w/$v.o" >/dev/null 2>&1 || continue
    LLRM_CC_TARGET=$tg fresh_compile "$w/$v.s" "$D/$v/llrm-cc" $fl $lvl ${CELLS_INC:-} -S "$f" -o "$w/$v.s" >/dev/null 2>&1 || continue
    [ -s "$w/$v.o" ] && [ -s "$w/$v.s" ] || continue
    # OMF (m16) objects defeat size(1): the file size then, symbols being equal on both sides.
    text[$v]=$(size "$w/$v.o" 2>/dev/null | awk 'NR==2{print $1}')
    [ -n "${text[$v]}" ] || text[$v]=$(stat -c %s "$w/$v.o")
    ins[$v]=$(grep -cE '^[[:space:]]+[a-z][a-z0-9.]*([[:space:]]|$)' "$w/$v.s")
  done
  if [ -z "${text[base]:-}" ] || [ -z "${text[new]:-}" ] || [ "${ins[base]:-0}" -eq 0 ] || [ "${ins[new]:-0}" -eq 0 ]; then
    echo "EMPTY $tg $lvl $f" ; empty=$((empty + 1)); unset 'text[base]' 'text[new]' 'ins[base]' 'ins[new]'; continue
  fi
  echo "$tg $lvl $(basename "$f") bytes ${text[base]} ${text[new]} instrs ${ins[base]} ${ins[new]}"
  unset 'text[base]' 'text[new]' 'ins[base]' 'ins[new]'
done < "$L"
# A cell that built nothing is a failed measurement, not a quiet one.
[ "$empty" -eq 0 ] || { echo "cells: $empty EMPTY cell(s)" >&2; exit 1; }
