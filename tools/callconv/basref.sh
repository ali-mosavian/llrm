#!/bin/bash
# Rebuilds BC's /A listings of the BASIC matrix, per compiler, into
# tests/fixtures/callconv/bas/bc/<dialect>: the reference the unit tests read.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
for d in vbdos pds71 qb45; do
    "$ROOT/tools/callconv/bas.sh" $d "$W/$d" > "$W/$d.log" 2>&1 &
done
wait
for d in vbdos pds71 qb45; do
    out="$ROOT/tests/fixtures/callconv/bas/bc/$d"
    rm -rf "$out"; mkdir -p "$out"
    for f in "$W/$d"/{CE,CES,CR,CRS,CUE,CVE}.LST; do [[ -f $f ]] && tr -d '\r' < "$f" > "$out/$(basename "$f")"; done
    grep -A3 "^== BB" "$W/$d.log"
done
