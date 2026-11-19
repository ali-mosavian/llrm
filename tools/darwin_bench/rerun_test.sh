#!/bin/bash
# A failing build must stop rerun.sh with its own message. It once printed "byte-identical ()" and went on to
# time nothing, because set -e is off inside $(build ...).
S=${1:-$(dirname "$0")/rerun.sh}; T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
git -C "$T" init -q r && git -C "$T/r" -c user.name=t -c user.email=t@t commit -q --allow-empty -m x
out=$(REPO=$T/r W=$T/w OUT=$T/o LLRM_PIN_CLANG=/bin/true bash "$S" HEAD HEAD O2 2>&1); rc=$?
[ $rc -ne 0 ] && echo "$out" | grep -q "build of .* failed" && ! echo "$out" | grep -q "byte-identical" \
  && echo PASS || { echo "FAIL rc=$rc"; echo "$out" | tail -3; exit 1; }
