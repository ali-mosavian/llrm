#!/usr/bin/env bash
# cells.sh DIR LIST: per-cell .text bytes and instruction count, DIR/base vs DIR/new.
# LIST lines: "<anything> <target> <-Olevel> <file.c>". Refuses to run when base and new are
# the same binaries (a sweep of a build against itself reads "no change" everywhere).
# Env: CELLS_FLAGS (extra cc flags), CELLS_INC (include flags).
set -u
D=$1; L=$2
[ -f "$D/base/llrm-cc" ] && [ -f "$D/new/llrm-cc" ] || { echo "cells: missing llrm-cc in base or new" >&2; exit 2; }
# A target's backend may be its own binary (llrm-wasm-llc), so the pair must differ somewhere, not in each file.
differs=0
for f in "$D"/base/*; do
  n="$D/new/$(basename "$f")"
  [ -f "$n" ] || { echo "cells: $(basename "$f") missing in new" >&2; exit 2; }
  [ "$(md5sum < "$f")" = "$(md5sum < "$n")" ] || differs=1
done
[ "$differs" = 1 ] || { echo "cells: base and new are the same binaries" >&2; exit 2; }
[ "$(readlink -f "$D/base")" != "$(readlink -f "$D/new")" ] || { echo "cells: base and new are one directory" >&2; exit 2; }
w=$(mktemp -d); trap 'rm -rf "$w"' EXIT
declare -A text ins
while read -r _ tg lvl f; do
  fl=""; case $tg in x86_16-wat) fl=-march=i386;; x86_32-linux) fl=-m32;; esac
  for v in base new; do
    rm -f "$w/$v.o" "$w/$v.s"
    LLRM_CC_TARGET=$tg "$D/$v/llrm-cc" $fl $lvl ${CELLS_INC:-} -c "$f" -o "$w/$v.o" >/dev/null 2>&1 || continue
    LLRM_CC_TARGET=$tg "$D/$v/llrm-cc" $fl $lvl ${CELLS_INC:-} -S "$f" -o "$w/$v.s" >/dev/null 2>&1
    [ -s "$w/$v.o" ] || continue
    # OMF (m16) objects defeat size(1): the file size then, symbols being equal on both sides.
    text[$v]=$(size "$w/$v.o" 2>/dev/null | awk 'NR==2{print $1}')
    [ -n "${text[$v]}" ] || text[$v]=$(stat -c %s "$w/$v.o")
    # Targets with no readable -S (wasm) are scored on bytes alone.
    if [ -s "$w/$v.s" ]; then ins[$v]=$(grep -cE '^[[:space:]]+[a-z][a-z0-9.]*([[:space:]]|$)' "$w/$v.s"); else ins[$v]=-; fi
    [ "${ins[$v]}" != 0 ] || ins[$v]=-
  done
  if [ -z "${text[base]:-}" ] || [ -z "${text[new]:-}" ] || [ -z "${ins[base]:-}" ] || [ -z "${ins[new]:-}" ]; then
    echo "EMPTY $tg $lvl $f" ; unset 'text[base]' 'text[new]' 'ins[base]' 'ins[new]'; continue
  fi
  echo "$tg $lvl $(basename "$f") bytes ${text[base]} ${text[new]} instrs ${ins[base]} ${ins[new]}"
  unset 'text[base]' 'text[new]' 'ins[base]' 'ins[new]'
done < "$L"
