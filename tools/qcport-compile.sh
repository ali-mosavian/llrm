#!/bin/bash
# Compile QCport's C modules with llrm-c at -O2 and -Os; list each one
# refused, exit 1 if any is. QCport is not in this repository: point
# QCPORT at its src/ and QCPORT_INC at the Borland headers it builds with.
#
#   QCPORT=~/scratch/qcport/src QCPORT_INC=~/scratch/qctc/inc tools/qcport-compile.sh [llrm-c]
export LLRM_VERIFY=${LLRM_VERIFY:-1}  # the gate checks each pass and phase
set -u
LLRM_C="${1:-$(python3 "$(dirname "$0")/llrmbin.py" bin)/llrm-c}"
QCPORT="${QCPORT:?QCPORT names the QCport src directory}"
QCPORT_INC="${QCPORT_INC:?QCPORT_INC names its Borland include directory}"
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT
includes=(); for d in host render model game sound ui qgl; do includes+=(-I "$QCPORT/$d"); done
includes+=(-I "$QCPORT_INC")
modules=$(ls "$QCPORT"/{host,render,model,game,sound,ui}/*.c)
refused=0
for level in -O2 -Os; do
    for source in $modules; do
        name=$(basename "$source" .c)
        if ! "$LLRM_C" "$level" "${includes[@]}" "$source" -o "$OUT/$name.obj" > "$OUT/$name.txt" 2>&1; then
            echo "$level $name: $(grep -o 'llrm-c: .*' "$OUT/$name.txt" | head -1)"
            refused=$((refused + 1))
        fi
    done
done
echo "$(echo "$modules" | wc -l) modules, $refused refusals"
[ "$refused" -eq 0 ]
