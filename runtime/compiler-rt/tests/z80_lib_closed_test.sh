#!/bin/bash
# z80_lib_closed.sh names a symbol the library references and nothing defines.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT
printf '\t.module t\n\t.area _CODE\n\t.globl __nowhere\nf:\tcall __nowhere\n' > $T/t.s; sdasz80 -g -o $T/t.rel $T/t.s
out=$(bash z80_lib_closed.sh $T || true)
[ "$out" = "FAIL: referenced, defined nowhere: __nowhere" ] || { echo "FAIL: $out"; exit 1; }
echo "ok"
