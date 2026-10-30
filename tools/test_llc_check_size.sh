#!/bin/sh
# fsum: clang -O2 folds the whole program to 31 bytes, so the clang column says nothing about codegen. The pin column
# (the pin's llc on the same IR) and the llrm column must read within 25% of each other.
# Needs LLRM_LLC to be an llrm-llc that compiles fsum (e.g. a wrapper running --isel=dag).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
row=$(LLRM_NO_BUILD=1 "$root/tools/llc_check.sh" fsum | awk '$1=="fsum"')
set -- $row
pin=$4; llrm=$5
[ "$pin" != - ] && [ "$llrm" != - ] || { echo "fsum: no size ($row)"; exit 1; }
[ $((llrm * 100)) -le $((pin * 125)) ] && [ $((pin * 100)) -le $((llrm * 125)) ] || { echo "fsum: pin $pin vs llrm $llrm"; exit 1; }
echo "ok: fsum pin $pin llrm $llrm"
