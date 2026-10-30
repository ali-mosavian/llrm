#!/usr/bin/env bash
# On an arm64 Mac: builds llrm-arm64-llc, compiles each tests/darwin/*.ll to a
# Mach-O object, links it with the system clang and runs it; its output and
# exit status must be the ones recorded in tests/darwin/*.out and *.status.
#   tools/targets/arm64-mac-check.sh
set -uo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
dir=$root/crates/target/llrm-arm64/tests/darwin
cargo build --release -p llrm-arm64 --bin llrm-arm64-llc || exit 1
llc=$(python3 "$root/tools/llrmbin.py" bin)/llrm-arm64-llc
tmp=$(mktemp -d)
failed=0
for ir in "$dir"/*.ll; do
  name=$(basename "$ir" .ll)
  if ! "$llc" -o "$tmp/$name.o" "$ir"; then echo "FAIL $name: compile"; failed=1; continue; fi
  if ! clang -arch arm64 -o "$tmp/$name" "$tmp/$name.o"; then echo "FAIL $name: link"; failed=1; continue; fi
  "$tmp/$name" > "$tmp/$name.out"; status=$?
  if cmp -s "$tmp/$name.out" "$dir/$name.out" && [ "$status" = "$(cat "$dir/$name.status")" ]; then
    echo "ok   $name"
  else
    echo "FAIL $name: output or status differs"; failed=1
  fi
done
exit $failed
