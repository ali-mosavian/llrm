#!/bin/bash
# Compile QCport's C modules with llrm-c at -O2 and -Os; list each one
# refused, exit 1 if any is. QCport is not in this repository: tools/corpora.py
# names its src/ and the Borland headers it builds with (the registered trees).
#
#   tools/qcport-compile.sh [llrm-c]
export LLRM_VERIFY=${LLRM_VERIFY:-1}  # the gate checks each pass and phase
set -u
LLRM_C="${1:-$(python3 "$(dirname "$0")/llrmbin.py" bin)/llrm-c}"
QCSRC="$(python3 "$(dirname "$0")/corpora.py" path qcport)"; QCINC="$(python3 "$(dirname "$0")/corpora.py" path qcport-inc)"  # the registered trees (tools/corpora.py)
OUT="$(mktemp -d)"; trap 'rm -rf "$OUT"' EXIT
includes=(); for d in host render model game sound ui qgl; do includes+=(-I "$QCSRC/$d"); done
includes+=(-I "$QCINC")
modules=$(ls "$QCSRC"/{host,render,model,game,sound,ui}/*.c)
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
