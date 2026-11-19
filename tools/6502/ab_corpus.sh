#!/bin/bash
# ab_corpus.sh CC OUTDIR [NAMES...]: the executed corpus A/B of the 6502 pass kill switch, ours_cr.py per
# CPU x level x (LLRM_6502_MCP=0 | default). Prints DONE only after every run exited 0 and wrote its table;
# a run that crashed is FAILED, and the runner exits 1. Run it under ~/scratch/lg/load_guard.sh.
# AB_CPUS (default "6502 65c02"), AB_LEVELS (default "O1 O2 Os"), AB_CORPUS_PY (default ours_cr.py) override.
set -u
cc=${1:?compiler (llrm-cc)}; out=${2:?output directory}; shift 2
if [ -z "${AB_CORPUS_PY:-}" ]; then  # ours_cr.py refuses a missing runtime in every run: say so once, before the 12
    [ -n "${OURS_RT:-}" ] || { echo "ab_corpus: OURS_RT is not set: name the mos_rt.s of the tree under test" >&2; exit 2; }
    [ -f "$OURS_RT" ] || { echo "ab_corpus: OURS_RT=$OURS_RT is not a file" >&2; exit 2; }
fi
py=${AB_CORPUS_PY:-$(dirname "$0")/ours_cr.py}
mkdir -p "$out"; runs=0; failed=0
for cpu in ${AB_CPUS:-6502 65c02}; do for level in ${AB_LEVELS:-O1 O2 Os}; do for mode in off on; do
    if [ $mode = off ]; then export LLRM_6502_MCP=0; else unset LLRM_6502_MCP; fi
    tag=$cpu-$level-$mode; runs=$((runs + 1))
    OURS_CPU=$cpu python3 "$py" "$cc" "$level" "$out/$tag.tsv" "$@" > "$out/$tag.log" 2>&1
    rc=$?
    if [ $rc -ne 0 ] || [ ! -s "$out/$tag.tsv" ]; then echo "FAILED $tag (exit $rc)"; failed=$((failed + 1)); fi
done; done; done
if [ $failed -eq 0 ]; then echo "DONE $runs runs"; exit 0; fi
echo "FAILED $failed of $runs runs; not done"; exit 1
