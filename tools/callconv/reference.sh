#!/bin/bash
# Rebuilds BCC 3.1's side of the C calling-convention matrix into
# tests/fixtures/callconv/c/bcc: -S listings of the plain callers and
# callees (the reference the unit tests read) and the objects the
# execution tests link. See tests/fixtures/callconv/readme.md.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$ROOT/tests/fixtures/callconv/c"
OUT="$SRC/bcc"
W="${KEEP:-$(mktemp -d)}"; [[ -n "${KEEP:-}" ]] || trap 'rm -rf "$W"' EXIT; mkdir -p "$W"
cp "$SRC"/*.c "$SRC"/*.h "$W/"
one() {
    local k=$1 jobs=()
    for pair in callee:caller aggee:agger; do
        ce=${pair%%:*}; cr=${pair##*:}
        # A near call must stay in one segment: each side is built into the other's.
        near=""; [[ $k == ?n ]] && near=1
        jobs+=("$k$ce.c -S" "$k$cr.c -S" "$k$ce.c")
        jobs+=("$k$cr.c -DARMED -r- ${near:+-zC${k^^}${ce^^}_TEXT}")
        [[ -n $near ]] && jobs+=("$k$ce.c -zC${k^^}${cr^^}_TEXT -o$k${ce:0:3}x.obj")
    done
    jobs+=("${k}harn.c")
    mkdir -p "$W/$k"
    cp "$W"/*.c "$W"/*.h "$W/$k/"
    "$ROOT/tools/callconv/bcc.sh" "$W/$k" "${jobs[@]}"
}
# One boot per convention, in parallel.
for k in cf cn pf pn; do one $k & done
wait
mkdir -p "$OUT"
rm -f "$OUT"/*
for f in "$W"/*/*.ASM "$W"/*/*.OBJ; do cp "$f" "$OUT/"; done
ls "$OUT"
