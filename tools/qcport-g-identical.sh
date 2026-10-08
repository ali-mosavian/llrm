#!/usr/bin/env bash
# qcport-g-identical.sh [llrm-c]: every QCport module at -O0, -O2 and -Os, assembly with and without -g; lists the ones that differ.
# Data labels are Watcom's handle numbers, which -g's debug records shift: they are compared without their numbers.
#   QCPORT=~/scratch/qcport/src QCPORT_INC=~/scratch/qctc/inc tools/qcport-g-identical.sh [llrm-c]
C=${1:-target/release/llrm-c}; Q=${QCPORT:?QCPORT names the QCport src directory}; INC=${QCPORT_INC:?QCPORT_INC names its Borland include directory}; T=$(mktemp -d)
inc=(); for d in host render model game sound ui qgl; do inc+=(-I "$Q/$d"); done; inc+=(-I "$INC")
same=0; differ=0; refused=0
for f in "$Q"/{host,render,model,game,sound,ui}/*.c; do n=$(basename "$f" .c)
  for level in -O0 -O2 -Os; do
    "$C" $level "${inc[@]}" -S "$f" -o "$T/a.s" >/dev/null 2>&1 || { refused=$((refused+1)); continue; }
    "$C" $level -g "${inc[@]}" -S "$f" -o "$T/b.s" >/dev/null 2>&1 || { echo "FAIL -g $n $level"; refused=$((refused+1)); continue; }
    if cmp -s <(sed -E 's/L_b[0-9]+/L_b/g' "$T/a.s") <(sed -E 's/L_b[0-9]+/L_b/g' "$T/b.s"); then same=$((same+1)); else differ=$((differ+1)); echo "DIFFERS $n $level"; fi
  done
done
echo "QCport: identical $same, differing $differ, refused $refused"; rm -rf "$T"
