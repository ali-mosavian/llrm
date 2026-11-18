#!/usr/bin/env bash
# Identity of two llrm-cc builds on one target and level: tools/identity_targets.sh BASE_CC NEW_CC TARGET LEVEL [FILE.c...]
# FILE defaults to tests/run/c/*.c. Prints the counts; exits 1 on a DIFF, a STATUS (one side refused) or an
# empty compared set (every program refused, or none given): that is no measurement, not a pass.
set -u
same=; [ "${1:-}" = --same-ok ] && { same=--same-ok; shift; }  # a head that changes only the frontend
base=${1:?base llrm-cc}; new=${2:?new llrm-cc}; target=${3:?target}; level=${4:?level}; shift 4
root=$(cd "$(dirname "$0")/.." && pwd)
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
same=0; diff=0; refused=0; status=0; na=0
for f in "$@"; do
    [ -f "$f" ] || continue
    # Each program is compiled as its own header says: a `// targets:` that excludes TARGET is n/a, not refused (36 of the 40 x86_64 "refusals" were DOS/m16 programs),
    # and the runtime include dirs are given (llrm_os.h).
    mapfile -t flags < <(python3 "$root/tools/corpus_refusals.py" --plan "$target" "$f")
    [ "${flags[0]}" = "n/a" ] && { na=$((na+1)); continue; }
    LLRM_CC_TARGET=$target "$base" -c "-$level" "${flags[@]}" "$f" -o "$out/a" > /dev/null 2>&1; a=$?
    LLRM_CC_TARGET=$target "$new" -c "-$level" "${flags[@]}" "$f" -o "$out/b" > /dev/null 2>&1; b=$?
    if [ $a != $b ]; then status=$((status+1)); echo "STATUS $f"
    elif [ $a != 0 ]; then refused=$((refused+1))
    elif cmp -s "$out/a" "$out/b"; then same=$((same+1))
    else diff=$((diff+1)); echo "DIFF $f"; fi
    rm -f "$out/a" "$out/b"
done
echo "$target $level same=$same diff=$diff refused=$refused status=$status na=$na"
[ $((same + diff)) -gt 0 ] && [ $diff = 0 ] && [ $status = 0 ]
