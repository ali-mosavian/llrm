#!/bin/bash
# The identity gate: two builds of a compiler make byte-identical objects for every program of a
# corpus. A change that must not move a target's output (a refactor, another target's work) runs
# this against a build of the base, and fails on any DIFF, or STATUS (one build refused what the
# other accepted). A program both refuse is REFUSED and counts for neither.
#
#   tools/identity.sh KIND BASE_BIN NEW_BIN [--target TARGET]
#
# KIND: nib    llrm-nib over examples/, tests/run/nib, bench/, at -O2 and -Os, procedure segments
#       c      llrm-c over bench/ and tests/run/c, at -O2 --cpu 486
#       qcport llrm-c over QCport's modules at -O2 and -Os; QCPORT names its src/ and QCPORT_INC the
#              Borland headers it builds with (neither is in this repository)
# TARGET is passed to the compiler (default: its own, code16).
#
# Prints a count of each outcome and the programs that are not SAME; exit 1 if any DIFF or STATUS.
set -u
kind=${1:?kind: nib, c or qcport}; base=${2:?base compiler}; new=${3:?new compiler}; shift 3
target=()
[ "${1:-}" = "--target" ] && target=(--target "${2:?target}")
root=$(cd "$(dirname "$0")/.." && pwd)
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
export base new out root
export target_flags="${target[*]:-}"

one() {
    source=$1; shift
    key=$(echo "$source $*" | tr '/ ' '__')
    "$base" "$source" "$@" $target_flags -o "$out/a_$key.obj" >/dev/null 2>&1; first=$?
    "$new" "$source" "$@" $target_flags -o "$out/b_$key.obj" >/dev/null 2>&1; second=$?
    if [ $first != $second ]; then echo "STATUS $source $* ($first, $second)"
    elif [ $first != 0 ]; then echo "REFUSED $source $*"
    elif cmp -s "$out/a_$key.obj" "$out/b_$key.obj"; then echo "SAME $source $*"
    else echo "DIFF $source $*"; fi
}
export -f one

cd "$root" || exit 2
case $kind in
nib)
    for level in -O2 -Os; do
        ls examples/*.nib tests/run/nib/*.nib bench/*/*.nib bench/parity/*/*.nib 2>/dev/null | sed "s|\$| $level --procedure-segments|"
    done ;;
c)
    ls bench/*/*.c tests/run/c/*.c | sed 's|$| -O2 --cpu 486|' ;;
qcport)
    : "${QCPORT:?QCPORT names the QCport src directory}" "${QCPORT_INC:?QCPORT_INC names its Borland include directory}"
    includes=""; for d in host render model game sound ui qgl; do includes="$includes -I $QCPORT/$d"; done
    for level in -O2 -Os; do for src in "$QCPORT"/{host,render,model,game,sound,ui}/*.c; do echo "$src $level $includes -I $QCPORT_INC"; done; done ;;
*) echo "identity.sh: unknown kind $kind" >&2; exit 2 ;;
esac | xargs -P 8 -L1 bash -c 'one $0 "$@"' | sort > "$out/results"
cut -d' ' -f1 "$out/results" | uniq -c
grep -v '^SAME' "$out/results"
! grep -q '^\(DIFF\|STATUS\)' "$out/results"
