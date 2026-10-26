#!/bin/sh
# tools/cc_check.sh [PROG...]: each tests/llc/PROG.c built by the pinned clang (host reference) and by llrm-cc for
# LLRM_CC_TARGET (table name, default x86_64-linux); both run, stdout and exit status compared. A target with a `run`
# recipe (DOS) runs its build under it and is checked against the host build of the same file; DOS programs print with
# `report(long)` set LLRM_CC_DIR to a directory of freestanding programs.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
clang=${LLRM_PIN_CLANG:-$HOME/scratch/llvm-build/bin/clang}
cc=${LLRM_CC:-$root/target/release/llrm-cc}
export LLRM_LLC=${LLRM_LLC:-$root/target/release/llrm-llc}
opt=${LLRM_CC_OPT:--O2}
dir=${LLRM_CC_DIR:-$root/tests/llc}
runner=${LLRM_CC_RUN:-}
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
progs=${*:-$(cd "$dir" && ls *.c | sed 's/\.c$//')}
bad=0
for p in $progs; do
  src=$dir/$p.c
  "$clang" $opt "$src" -o "$out/ref" -lm 2>/dev/null || { echo "$p: reference failed"; bad=1; continue; }
  if "$cc" $opt "$src" -o "$out/llrm.exe" -lm 2> "$out/err"; then
    "$out/ref" > "$out/ref.out" 2>/dev/null; rr=$?
    $runner "$out/llrm.exe" > "$out/llrm.out" 2>/dev/null; lr=$?
    if [ $rr = $lr ] && cmp -s "$out/ref.out" "$out/llrm.out"; then echo "ok    $p"; else echo "DIFF  $p"; bad=1; fi
  else echo "FAIL  $p: $(head -c 200 "$out/err")"; bad=1; fi
done
exit $bad
