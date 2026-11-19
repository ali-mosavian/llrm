#!/bin/bash
# usage: z80_lib_closed.sh DIR -> exit 1 naming each symbol a .rel in DIR references that no .rel in DIR, the runtime (z80_rt.s) or the C library entries define.
# A f64 multiply once needed __umulhisi3, which only int32.s defined and the library never shipped: programs that multiply doubles failed to link (11-07).
DIR=${1:?library directory}
cd "$(dirname "$0")/../../.."
T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT
sdasz80 -g -o $T/rt.rel crates/target/llrm-z80/runtime/z80_rt.s
for s in crates/target/llrm-z80/tests/programs/libc/*.s; do sdasz80 -g -o $T/$(basename $s .s).rel $s; done
cat "$DIR"/*.rel $T/*.rel | awk '$1=="S" && $3 ~ /^Def/ {d[$2]=1} $1=="S" && $3 ~ /^Ref/ {r[$2]=1} END {for (s in r) if (!(s in d)) print s}' | sort > $T/missing
[ ! -s $T/missing ] || { echo "FAIL: referenced, defined nowhere: $(paste -sd' ' $T/missing)"; exit 1; }
