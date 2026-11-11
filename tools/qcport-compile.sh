#!/bin/bash
# Compile QCport's C modules with llrm-cc at -O2 and -Os; list each one
# refused, exit 1 if any is. QCport is not in this repository: point
# QCPORT at its src/ and QCPORT_INC at the Borland headers it builds with.
#
#   QCPORT=~/scratch/qcport/src QCPORT_INC=~/scratch/qctc/inc tools/qcport-compile.sh [llrm-cc]
export LLRM_VERIFY=${LLRM_VERIFY:-1}  # the gate checks each pass and phase
set -u
LLRM_C="${1:-$(python3 "$(dirname "$0")/llrmbin.py" bin)/llrm-cc}"
QCPORT="${QCPORT:?QCPORT names the QCport src directory}"
QCPORT_INC="${QCPORT_INC:?QCPORT_INC names its Borland include directory}"
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT
# llrm-cc's default target is x86_64-linux, where QCport's Borland sources do not parse: name the m16 row (as tools/oracle_qcport.py) and Borland's headers
# as clang reads them (lower-case names, no 0x1A end-of-file byte).
export LLRM_CC_TARGET="${LLRM_CC_TARGET:-x86_16-dos}"
"$(dirname "$0")/lowercase-includes.sh" "$QCPORT_INC" "$OUT/inc" || exit 2
includes=(-c -fasm-blocks -D_asm=__asm -nostdinc -isystem "$OUT/inc"); for d in host render model game sound ui qgl; do includes+=(-I "$QCPORT/$d"); done
modules=$(ls "$QCPORT"/{host,render,model,game,sound,ui}/*.c 2>/dev/null)
[ -n "$modules" ] || { echo "qcport-compile: no modules under $QCPORT" >&2; exit 2; }
refused=0
for level in -O2 -Os; do
    for source in $modules; do
        name=$(basename "$source" .c)
        if ! "$LLRM_C" "$level" "${includes[@]}" "$source" -o "$OUT/$name.obj" > "$OUT/$name.txt" 2>&1; then
            echo "$level $name: $(grep -o 'llrm-cc: .*' "$OUT/$name.txt" | head -1)"
            refused=$((refused + 1))
        fi
    done
done
echo "$(echo "$modules" | wc -l) modules, $refused refusals"
[ "$refused" -eq 0 ]
