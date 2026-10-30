#!/bin/sh
# lle_expected.sh [FILE.ll...]: writes FILE.expected, the pinned LLVM's `opt -passes=loop-load-elim -S` of each
# crates/opt/llrm-transforms/tests/lle/*.ll (or the named files), which looploadelim_tests.rs compares with.
OPT=${LLVM_OPT:-$HOME/scratch/llvm-build/bin/opt}
DIR=$(dirname "$0")/../../crates/opt/llrm-transforms/tests/lle
[ $# -gt 0 ] || set -- "$DIR"/*.ll
for file in "$@"; do
  "$OPT" -passes=loop-load-elim -S "$file" | grep -v '^; ModuleID\|^source_filename' > "${file%.ll}.expected" || exit 1
done
