#!/bin/sh
# Compile every example C file with llrm-cc for each target: tools/cc-sweep.sh [target...]  (LLRM_CLANG_M16 for x86_16-dos)
root=$(cd "$(dirname "$0")/.." && pwd)
targets=${*:-x86_16-dos x86_32-dos x86_64-linux arm64-linux}
bin=$(python3 "$root/tools/llrmbin.py" bin)
out=${CC_SWEEP_OUT:-/tmp/cc-sweep}; mkdir -p "$out"
export LLRM_LLC=$bin/llrm-llc
for t in $targets; do
  find "$root/examples" "$root/bench/parity" "$root/tests/llc" "$root/crates/target/llrm-arm64/tests/programs" -name '*.c' |
    xargs -P 6 -I{} sh -c 'f={}; n=$(echo "$f" | md5sum | cut -c1-8)_$(basename "$f" .c); LLRM_CC_TARGET='$t' '"$bin"'/llrm-cc -O1 -c "$f" -o '$out'/'$t'.$n.o 2> '$out'/'$t'.$n.err && echo "ok $f" || echo "FAIL $f"' > "$out/$t.txt"
  echo "$t: ok $(grep -c '^ok' "$out/$t.txt") fail $(grep -c '^FAIL' "$out/$t.txt")"
done
