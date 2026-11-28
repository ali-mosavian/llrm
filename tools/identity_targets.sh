#!/usr/bin/env bash
# Identity of two llrm-cc builds on one target and level: tools/identity_targets.sh BASE_CC NEW_CC TARGET LEVEL [FILE.c...]
# FILE defaults to tests/run/c/*.c. Prints the counts; exits 1 on a DIFF, a STATUS (one side refused) or an
# empty compared set (every program refused, or none given): that is no measurement, not a pass.
set -u
same=; [ "${1:-}" = --same-ok ] && { same=--same-ok; shift; }  # a head that changes only the frontend
base=${1:?base llrm-cc}; new=${2:?new llrm-cc}; target=${3:?target}; level=${4:?level}; shift 4
root=$(cd "$(dirname "$0")/.." && pwd)
# Each side is the commit LLRM_EXPECT_BASE_SHA / LLRM_EXPECT_SHA names, built clean (stamp.py require_named).
python3 "$root/tools/stamp.py" --require-clean base_sha "$(dirname "$base")" && python3 "$root/tools/stamp.py" --require-clean sha "$(dirname "$new")" || exit 2
# The segmented rows need the fork clang of toolchain/clang/pin (line 2, as tools/llrmbin.py finds it); without it llrm-cc refuses every program
# (x86_16-dos refused=215 on #10190 and #10193, read as "unmeasurable").
case $target in
x86_16-* | x86_32-dos | x86_32-wat)
    m16=$HOME/scratch/clang-by-sha/$(sed -n 2p "$root/toolchain/clang/pin" | cut -d' ' -f1)/bin/clang
    [ -n "${LLRM_CLANG_M16:-}" ] || ! [ -x "$m16" ] || export LLRM_CLANG_M16=$m16 ;;
esac
python3 "$root/tools/llrmbin.py" sides "$base" "$new" $same --target "$target" || exit 2  # prints cc + llc md5 per side; refuses one llc
[ $# -gt 0 ] || set -- "$root"/tests/run/c/*.c
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
same=0; diff=0; refused=0; status=0; hung=0
# A compile that never ends (z80 -O3 known_loads_walks, 2.5 h) is no result: it ends at IDENTITY_TIMEOUT s and is counted apart.
limit=${IDENTITY_TIMEOUT:-120}
for f in "$@"; do
    [ -f "$f" ] || continue
    LLRM_CC_TARGET=$target timeout "$limit" "$base" -c "-$level" "$f" -o "$out/a" > /dev/null 2>&1; a=$?
    LLRM_CC_TARGET=$target timeout "$limit" "$new" -c "-$level" "$f" -o "$out/b" > /dev/null 2>&1; b=$?
    if [ $a = 124 ] || [ $b = 124 ]; then hung=$((hung+1)); echo "HUNG $f (base $a, new $b)"
    elif [ $a != $b ]; then status=$((status+1)); echo "STATUS $f"
    elif [ $a != 0 ]; then refused=$((refused+1))
    elif cmp -s "$out/a" "$out/b"; then same=$((same+1))
    else diff=$((diff+1)); echo "DIFF $f"; fi
    rm -f "$out/a" "$out/b"
done
echo "$target $level same=$same diff=$diff refused=$refused status=$status hung=$hung"
[ $((same + diff)) -gt 0 ] && [ $diff = 0 ] && [ $status = 0 ] && [ $hung = 0 ]
