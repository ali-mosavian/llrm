#!/bin/sh
# tools/lowercase-includes.sh SRC DST: DST holds a lower-case symlink for every file under SRC beside the original name,
# so DOS-cased headers (ALLOC.H) are found by `#include <alloc.h>` and `<ALLOC.H>` on a case-sensitive host. The copies
# lose DOS's end-of-file byte (0x1A), which clang reads as a stray character.
set -eu
src=$(cd "$1" && pwd); dst=$2
rm -rf "$dst"; mkdir -p "$dst"
(cd "$src" && find . -type f) | while read -r file; do
  lower=$(printf '%s' "$file" | tr 'A-Z' 'a-z')
  mkdir -p "$dst/$(dirname "$lower")"
  tr -d '\032' <"$src/$file" >"$dst/$lower"
  [ "$lower" = "$file" ] || ln -s "$lower" "$dst/$file"
done
[ -n "$(find "$dst" -type f -print -quit)" ] || { echo "lowercase-includes: $src holds no files; $dst is empty" >&2; exit 1; }
