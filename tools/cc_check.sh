#!/bin/sh
# tools/cc_check.sh [PROG...]: each tests/llc/PROG.c built by the pinned clang (reference) and by llrm-cc, both run on
# this host; prints whether stdout and exit status agree. Env: LLRM_CC_TARGET (table name), LLRM_CC_OPT (default -O2).
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
clang=${LLRM_PIN_CLANG:-$HOME/scratch/llvm-build/bin/clang}
cc=${LLRM_CC:-$root/target/release/llrm-cc}
export LLRM_LLC=${LLRM_LLC:-$root/target/release/llrm-llc}
opt=${LLRM_CC_OPT:--O2}
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
progs=${*:-$(cd "$root/tests/llc" && ls *.c | sed 's/\.c$//')}
bad=0
for p in $progs; do
  src=$root/tests/llc/$p.c
  "$clang" $opt "$src" -o "$out/ref" -lm 2>/dev/null || { echo "$p: reference failed"; bad=1; continue; }
  if "$cc" $opt "$src" -o "$out/llrm" -lm 2> "$out/err"; then
    "$out/ref" > "$out/ref.out" 2>&1; rr=$?
    "$out/llrm" > "$out/llrm.out" 2>&1; lr=$?
    if [ $rr = $lr ] && cmp -s "$out/ref.out" "$out/llrm.out"; then echo "ok    $p"; else echo "DIFF  $p"; bad=1; fi
  else echo "FAIL  $p: $(head -c 200 "$out/err")"; bad=1; fi
done
exit $bad
