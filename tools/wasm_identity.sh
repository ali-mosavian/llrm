#!/bin/bash
# The wasm32 identity gate: two dists compile bench/ and tests/run/c at O1 O2 O3 Os with llrm-cc, and the objects are compared.
#
#   tools/wasm_identity.sh BASE_DIR HEAD_DIR OUT   (each dir holds llrm-cc, llrm-llc and llrm-wasm-llc)
#
# Rows in OUT/rows.txt: SAME|DIFF wasm32 LEVEL NAME, NA (the front end rejects the source: DOS-only keywords, a missing header),
# FAIL wasm32 LEVEL NAME RC_BASE RC_HEAD (a backend refused it). Refuses two dists that share an llrm-llc or an llrm-wasm-llc.
# Exit 1 if any DIFF, any FAIL whose two codes differ or are not a refusal (1, 2: 127, 124, a signal is a broken tool), or no SAME row. IDENTITY_SAME_LLC=1 allows a head that changes only the frontend.
set -u
base=${1:?base dir}; new=${2:?head dir}; out=${3:?out dir}
root=$(cd "$(dirname "$0")/.." && pwd)
python3 -c "import sys; sys.path.insert(0, '$root/tools'); import llrmbin; llrmbin.assert_distinct_llc(sys.argv[1] + '/llrm-cc', sys.argv[2] + '/llrm-cc', bool(sys.argv[3]), ('llrm-llc', 'llrm-wasm-llc'), True)" "$base" "$new" "${IDENTITY_SAME_LLC:-}" || exit 2
mkdir -p "$out"; rm -f "$out/rows.txt"
export LLRM_CLANG_WASM=${LLRM_CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang} LLRM_CC_TARGET=wasm32 base new out root
cd "$root" || exit 2
one() {
    level=$1; source=$2; name=$(echo "$source" | tr '/' '_' | sed 's/\.c$//')
    for side in base new; do
        dir=${!side}
        LLRM_LLC=$dir/llrm-llc timeout 300 "$dir/llrm-cc" -"$level" -c -o "$out/$level.$name.$side.o" "$source" -Ibench -Iruntime/shared/posix > "$out/$level.$name.$side.err" 2>&1
        echo $? > "$out/$level.$name.$side.rc"
    done
    rb=$(cat "$out/$level.$name.base.rc"); rh=$(cat "$out/$level.$name.new.rc")
    if [ "$rb" = 0 ] && [ "$rh" = 0 ]; then
        if cmp -s "$out/$level.$name.base.o" "$out/$level.$name.new.o"; then echo "SAME wasm32 $level $name"; else echo "DIFF wasm32 $level $name"; fi
    elif [ "$(cat "$out/$level.$name.base.err" "$out/$level.$name.new.err" | python3 -I "$root/tools/sweepclass.py")" = NA ]; then echo "NA wasm32 $level $name"
    else echo "FAIL wasm32 $level $name $rb $rh"; fi
    rm -f "$out/$level.$name".*
}
export -f one
for level in O1 O2 O3 Os; do ls bench/*/*.c tests/run/c/*.c | sed "s|^|$level |"; done | xargs -P "${JOBS:-4}" -L 1 bash -c 'one $0 $1' | sort > "$out/rows.txt"
cut -d' ' -f1 "$out/rows.txt" | uniq -c
grep -v '^\(SAME\|NA\)' "$out/rows.txt"
# Fails on a DIFF, a FAIL whose codes differ, a FAIL with any code but a refusal (1, 2), or no SAME row (nothing compared).
awk '$1=="DIFF"{bad=1} $1=="FAIL" && ($5!=$6 || $5<1 || $5>2){bad=1} $1=="SAME"{same=1} END{exit (bad || !same)}' "$out/rows.txt"
