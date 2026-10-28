#!/bin/sh
# tools/cc_check.sh [PROG...]: each tests/llc/PROG.c built by the pinned clang (host reference) and by llrm-cc for
# LLRM_CC_TARGET (table name, default x86_64-linux); both run, stdout and exit status compared. A target with a `run`
# recipe (DOS) runs its build under it and is checked against the host build of the same file; DOS programs print with
# `report(long)` set LLRM_CC_DIR to a directory of freestanding programs.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
# The segmented DOS/Watcom/Borland rows read `#pragma aux` link names only in the pinned fork clang (toolchain/clang/pin, line 2).
# Without it libm names stay unmangled and every program fails to link (`_atan` undefined): stop here with one line instead.
case ${LLRM_CC_TARGET:-} in
x86_16-* | x86_32-dos | x86_32-wat)
  want=$(sed -n 2p "$root/toolchain/clang/pin" | cut -c1-12)
  have=$("${LLRM_CLANG_M16:-/nonexistent}" --version 2>/dev/null | grep -o '[0-9a-f]\{40\})' | cut -c1-12)
  if [ "$have" != "$want" ]; then
    echo "cc_check: $LLRM_CC_TARGET needs LLRM_CLANG_M16 = the fork clang at $want (toolchain/clang/pin); got '${LLRM_CLANG_M16:-unset}' at '${have:-none}'" >&2
    exit 2
  fi ;;
esac
clang=${LLRM_PIN_CLANG:-$HOME/scratch/llvm-build/bin/clang}
bin=$(python3 "$root/tools/llrmbin.py" bin)
cc=${LLRM_CC:-$bin/llrm-cc}
export LLRM_LLC=${LLRM_LLC:-$bin/llrm-llc}
opt=${LLRM_CC_OPT:--O2}
dir=${LLRM_CC_DIR:-$root/tests/llc}
runner=${LLRM_CC_RUN:-}
# LLRM_CC_REF_FLAGS: flags for the host reference (-m32 for x86_32-linux).
# A reference that is not the host's (arm64 under qemu): LLRM_CC_REF_CLANG -target LLRM_CC_REF_TRIPLE, linked by LLRM_CC_REF_LINK.
reftriple=${LLRM_CC_REF_TRIPLE:-}
refrun=; [ -z "$reftriple" ] || refrun=$runner
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
progs=${*:-$(cd "$dir" && ls *.c | sed 's/\.c$//')}
bad=0
for p in $progs; do
  src=$dir/$p.c
  # A program that reports through `extern void report(long)` gets the one printing it with a library's printf.
  extra=; grep -q "extern void report" "$src" && extra=$root/tests/cc/report.c
  # `far`/`near`/`huge` mean something only on the segmented target; elsewhere they are empty, as in a flat C compiler.
  case ${LLRM_CC_TARGET:-} in x86_16*) farless= ;; *) farless="-Dfar= -Dnear= -Dhuge=" ;; esac
  if [ -n "$reftriple" ]; then
    "${LLRM_CC_REF_CLANG:-$clang}" -target "$reftriple" -ffreestanding -fno-builtin $opt -c "$src" -o "$out/ref.o" 2>/dev/null &&
      $LLRM_CC_REF_LINK "$out/ref" "$out/ref.o" 2>/dev/null || { echo "$p: reference failed"; bad=1; continue; }
  else
    "$clang" ${LLRM_CC_REF_FLAGS:-} $opt -Dfar= -Dnear= -Dhuge= "$src" $extra -o "$out/ref" -lm 2>/dev/null || { echo "$p: reference failed"; bad=1; continue; }
  fi
  if "$cc" $opt $farless "$src" $extra -o "$out/llrm.exe" -lm 2> "$out/err"; then
    $refrun "$out/ref" > "$out/ref.out" 2>/dev/null; rr=$?
    $runner "$out/llrm.exe" > "$out/llrm.out" 2>/dev/null; lr=$?
    if [ $rr = $lr ] && cmp -s "$out/ref.out" "$out/llrm.out"; then echo "ok    $p"; else echo "DIFF  $p"; bad=1; fi
  else echo "FAIL  $p: $(head -c 200 "$out/err")"; bad=1; fi
done
exit $bad
