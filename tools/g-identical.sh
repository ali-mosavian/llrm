#!/usr/bin/env bash
# g-identical.sh llrm-c "mode:format ...": the assembly of every bench C program with and without -g, per target and level;
# lists the ones that differ. The targets are the pairs -g ships, for example "m32:elf".
C=$1; TARGETS=$2; T=$(mktemp -d); same=0; differ=0
for f in bench/*/*.c; do n=$(basename "$f" .c)
  for t in $TARGETS; do m=${t%%:*}; fmt=${t##*:}; for o in O0 O1 O2 Os; do
    $C -$m -fobject-format=$fmt -$o -S "$f" -o "$T/a.s" >/dev/null 2>&1 || continue
    $C -$m -fobject-format=$fmt -$o -g -S "$f" -o "$T/b.s" >/dev/null 2>&1 || { echo "FAIL -g $n $t $o"; continue; }
    if cmp -s "$T/a.s" "$T/b.s"; then same=$((same+1)); else differ=$((differ+1)); echo "DIFFERS $n $t $o"; fi
  done; done
done
echo "identical $same, differing $differ"; rm -rf "$T"
