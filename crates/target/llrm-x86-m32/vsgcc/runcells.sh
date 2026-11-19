#!/bin/bash
# runcells.sh OUT.jsonl PROG...: build PROG, run it at the 12 variants, one harness row per variant in OUT.jsonl (the RESULTS.jsonl matrix.py reads).
# A program that does not build or a variant that gives no row prints `FAILED PROG VARIANT: reason` and the script exits 1 at its end.
# One program per call, so parallel callers (xargs -P) must write distinct OUT files.
out=$1; shift
[ -n "$out" ] && [ $# -gt 0 ] || { echo "usage: runcells.sh OUT.jsonl PROG..." >&2; exit 2; }
H=$(cd "$(dirname "$0")" && pwd); R=$(cd "$H/../../../.." && pwd); rc=0
: > "$out"
for p in "$@"; do
  log=$(mktemp)
  if ! bash "$H/build.sh" "$p" >"$log" 2>&1; then echo "FAILED $p build: $(tail -1 "$log")"; rc=1; rm -f "$log"; continue; fi
  for v in llrmO1 gccO1 clangO1 llrm gccO2 clangO2 llrmO3 gccO3 clangO3 llrmOs gccOs clangOs; do
    line=$(uv run --project "$R/tools" python "$H/harness.py" "$p" "$v" 2>"$log" | tail -1)
    if printf '%s' "$line" | python3 -c "import sys,json; json.loads(sys.stdin.read())" 2>/dev/null; then
      echo "$line" >> "$out"
    else
      echo "FAILED $p $v: $(tail -1 "$log")"; rc=1
    fi
  done
  rm -f "$log"
done
exit $rc
