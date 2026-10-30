#!/usr/bin/env bash
# Clang split oracle: `clang -disable-llvm-passes` + `opt default<On>` must print
# the same IR as `clang -On` (.bc bytes differ by writer; Os/Oz run default<O2>,
# the optsize attribute carries the size level). Usage: clang-oracle.sh [file.c...]
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
bin=${LLVM_BIN:-$HOME/scratch/llvm-build/bin}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
files=("$@"); [ ${#files[@]} -eq 0 ] && files=("$root"/bench/parity/*/*.c)
fail=0; ran=0
for f in "${files[@]}"; do
  "$bin/clang" -fsyntax-only "$f" 2>/dev/null || { echo "SKIP  $(basename "$f") (host clang rejects)"; continue; }
  for o in O0 O1 O2 Os Oz; do
    "$bin/clang" -$o -c -emit-llvm -fdiscard-value-names -o "$tmp/ref.bc" "$f" 2>/dev/null &&
    "$bin/clang" -$o -c -emit-llvm -fdiscard-value-names -Xclang -disable-llvm-passes -o "$tmp/raw.bc" "$f" 2>/dev/null &&
    p=$o; case $o in Os|Oz) p=O2;; esac
    "$bin/opt" -discard-value-names -passes="default<$p>" "$tmp/raw.bc" -o "$tmp/opt.bc" 2>/dev/null &&
    "$bin/opt" -S "$tmp/ref.bc" 2>/dev/null | grep -v '^; ModuleID' > "$tmp/ref.ll" &&
    "$bin/opt" -S "$tmp/opt.bc" 2>/dev/null | grep -v '^; ModuleID' > "$tmp/opt.ll"
    if [ $? -ne 0 ]; then echo "ERROR $(basename "$f") -$o"; fail=1
    elif cmp -s "$tmp/ref.ll" "$tmp/opt.ll"; then ran=$((ran + 1)); echo "ok    $(basename "$f") -$o"
    else echo "DIFF  $(basename "$f") -$o"; fail=1; fi
  done
done
echo "$ran identical"
[ "$ran" -gt 0 ] || { echo "no file compared" >&2; fail=1; }
exit $fail
