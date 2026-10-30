#!/bin/sh
# laa_expected.sh [FILE.ll...]: writes FILE.expected, the pinned LLVM's `opt -passes='print<access-info>'` of each
# crates/opt/llrm-analysis/tests/laa/*.ll (or the named files), which loopaccessinfo_tests.rs compares with.
OPT=${LLVM_OPT:-$HOME/scratch/llvm-build/bin/opt}
DIR=$(dirname "$0")/../../crates/opt/llrm-analysis/tests/laa
[ $# -gt 0 ] || set -- "$DIR"/*.ll
for file in "$@"; do
  "$OPT" -disable-output -passes='print<access-info>' "$file" > "${file%.ll}.expected" 2>&1 || exit 1
done
