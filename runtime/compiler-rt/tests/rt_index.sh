#!/bin/sh
# usage: tests/rt_index.sh GROUP... -> "symbol<TAB>file<TAB>needs": each global symbol the group's members define, its source file, and the symbols it
# calls that another member of the same group defines. A resolver pulls the member of an undefined symbol, then its needs.
cd "$(dirname "$0")/.."; T=$(mktemp -d); trap 'rm -r $T' EXIT; : > $T/def; : > $T/und
# RT_CFLAGS: extra compiler flags (the target config); RT_EXTRA: sources the config adds (6502: config/6502/sf32_6502.c).
for f in $(tests/fp_sources.sh "$@") $RT_EXTRA; do
  o=$T/$(echo $f | tr / _).o; cc -c -O2 -fno-builtin -Iinclude -DSF_INTABI $RT_CFLAGS "$f" -o $o || exit 1
  nm --defined-only -g $o | awk -v f=$f '{print $3 "\t" f}' >> $T/def
  nm -u $o | awk -v f=$f '{print f "\t" $2}' >> $T/und
done
while IFS="$(printf '\t')" read -r sym file; do
  n=$(awk -F'\t' -v f="$file" 'NR == FNR { def[$1] = $2; next } $1 == f && ($2 in def) && def[$2] != f { printf "%s%s", s, $2; s = "," }' $T/def $T/und)
  printf '%s\t%s\t%s\n' "$sym" "$file" "$n"
done < $T/def
