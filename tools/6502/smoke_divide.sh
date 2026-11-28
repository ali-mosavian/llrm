#!/bin/bash
# sim65 smoke for 6502 wrong code from a call's clobbers against copy folding: six small C programs
# (smoke/*.c: divide pairs, a divide after a pair, a multiply after a divide, div_constants_short cut down)
# compiled at -O2 and run; each must print the numbers in smoke/NAME.out, which the host's gcc -m32 makes.
#
# usage: tools/6502/smoke_divide.sh [--regen]     (--regen rewrites smoke/*.out from the host's gcc)
# env:   LLRM_CC  llrm-cc to test (default: the one tools/llrmbin.py resolves);
#                 the llrm-6502-llc must sit beside it
#        OURS_RT  the mos_rt.s of the tree under test (default: this checkout's)
# exit:  0 every program matches; 1 a program is wrong, refused, does not link or halt, or its .out
#        is not the host's (one line "FAIL name: why" each); 2 the setup is missing a tool or binary
set -u
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
smoke=$here/smoke
cc=${LLRM_CC:-$(python3 "$repo/tools/llrmbin.py" bin)/llrm-cc}
export OURS_RT=${OURS_RT:-$repo/crates/target/llrm-6502/runtime/mos_rt.s}
programs=$(cd "$smoke" && ls *.c | sed 's/\.c$//')
work=$(mktemp -d "${TMPDIR:-/tmp}/smoke-divide.XXXXXX")
trap 'rm -rf "$work"' EXIT

# the host's numbers: report() prints each value, one per line, as a 32-bit long
host_run() {
    printf '#include <stdio.h>\nvoid report(long v) { printf("%%ld\\n", v); }\n' > "$work/report.c"
    gcc -m32 -O1 -w -fwrapv "$smoke/$1.c" "$work/report.c" -o "$work/host-$1" 2> "$work/host-$1.err" || return 1
    "$work/host-$1" | tr '\n' ' ' | sed 's/ $//'
    echo
}

if [ "${1:-}" = "--regen" ]; then
    for p in $programs; do
        host_run "$p" > "$smoke/$p.out" || { echo "FAIL $p: gcc -m32 failed"; exit 2; }
    done
    exit 0
fi

for need in python3 cl65 sim65; do
    command -v "$need" > /dev/null || { echo "setup: $need not found"; exit 2; }
done
[ -x "$cc" ] || { echo "setup: no llrm-cc at $cc (build it, or set LLRM_CC)"; exit 2; }
[ -x "$(dirname "$cc")/llrm-6502-llc" ] || { echo "setup: no llrm-6502-llc beside $cc"; exit 2; }
[ -f "$OURS_RT" ] || { echo "setup: OURS_RT=$OURS_RT is not a file"; exit 2; }

fail=0
OURS_CORPUS=$smoke python3 "$here/ours_cr.py" "$cc" O2 "$work/o.tsv" $programs > "$work/run.log" 2>&1
[ -f "$work/o.tsv.json" ] || { echo "setup: ours_cr.py did not finish"; tail -5 "$work/run.log"; exit 2; }
python3 - "$work/o.tsv.json" <<'PY' || fail=1
import json, sys
bad = [r for r in json.load(open(sys.argv[1])) if r[1] != "PASS"]
for name, status, why in bad:
    print(f"FAIL {name}: {status} {why}")
sys.exit(1 if bad else 0)
PY

if gcc -m32 -x c /dev/null -c -o /dev/null 2> /dev/null; then
    for p in $programs; do
        [ "$(host_run "$p")" = "$(cat "$smoke/$p.out")" ] || { echo "FAIL $p: smoke/$p.out is not what the host's gcc prints"; fail=1; }
    done
else
    echo "note: gcc -m32 missing, the .out files were not checked against the host" >&2
fi
exit $fail
