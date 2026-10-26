#!/bin/sh
# tools/lowercase-includes.sh SRC DST: DST holds a lower-case symlink for every file under SRC, so DOS-cased headers
# (ALLOC.H) are found by `#include <alloc.h>` on a case-sensitive host.
set -eu
src=$(cd "$1" && pwd); dst=$2
rm -rf "$dst"; mkdir -p "$dst"
(cd "$src" && find . -type f) | while read -r file; do
  lower=$(printf '%s' "$file" | tr 'A-Z' 'a-z')
  mkdir -p "$dst/$(dirname "$lower")"
  ln -s "$src/$file" "$dst/$lower"
done
