#!/bin/bash
# Identity of non-f128 output: tests/run/c compiled by a base and a new llrm-cc dir, per target, -O2 -c; objects compared.
# usage: f128_identity.sh BASE_BIN_DIR NEW_BIN_DIR [TARGET...]
set -u
base=${1:?base bin dir}; new=${2:?new bin dir}; shift 2
root=$(cd "$(dirname "$0")/.." && pwd); out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
ulimit -v 16000000
for t in ${*:-x86_64-linux x86_32-linux arm64-linux wasm32 m68k-68020 6502-sim65 z80-sdcc}; do
  same=0; diff=0; one=0; both=0; n=0
  for f in "$root"/tests/run/c/*.c; do
    b=$(basename "$f" .c); n=$((n+1))
    for w in base new; do
      d=$base; [ $w = new ] && d=$new
      LLRM_LLC=$d/llrm-llc LLRM_CC_TARGET=$t timeout 300 "$d/llrm-cc" -O2 -c "$f" -I"$root/runtime/shared/posix" -o "$out/$w.o" 2>/dev/null && ok=1 || ok=0
      eval "ok_$w=$ok"; [ $ok = 1 ] && mv "$out/$w.o" "$out/$w.$b.o"
    done
    if [ $ok_base = 1 ] && [ $ok_new = 1 ]; then
      if cmp -s "$out/base.$b.o" "$out/new.$b.o"; then same=$((same+1)); else diff=$((diff+1)); echo "DIFF $t $b"; fi
    elif [ $ok_base = 1 ] || [ $ok_new = 1 ]; then one=$((one+1)); echo "STATUS $t $b base=$ok_base new=$ok_new"
    else both=$((both+1)); fi
    rm -f "$out"/*.o
  done
  echo "$t: programs $n same $same DIFF $diff STATUS $one refused-by-both $both"
done
