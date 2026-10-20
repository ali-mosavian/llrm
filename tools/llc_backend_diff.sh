#!/bin/sh
# tools/llc_backend_diff.sh [PROG...]: the backend alone. llrm's IR after its mid-end (the switchconv stage) is
# compiled by llrm-llc and by the pinned llc -O2; prints .text bytes and instruction counts of each.
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
b=${LLRM_PIN_BIN:-$HOME/scratch/llvm-build/bin}
llrm=${LLRM_LLC:-$root/target/debug/llrm-llc}
triple=${LLRM_TRIPLE:-x86_64-linux-gnu}
out=$(mktemp -d)
progs=${*:-$(cd "$root/tests/llc" && ls *.c | sed 's/\.c$//')}
printf '%-10s %7s %7s   %7s %7s  (.text bytes | instructions)\n' prog llc llrm llc llrm
for p in $progs; do
  "$b/clang" -target $triple -O2 -Xclang -disable-llvm-passes -S -emit-llvm "$root/tests/llc/$p.c" -o "$out/$p.ll"
  LLRM_MIR_STAGES="$out/$p.d" "$llrm" -O2 "$out/$p.ll" -o "$out/$p.llrm.o" 2> "$out/$p.err" || { echo "$p: llrm failed"; continue; }
  stage=$(ls "$out/$p.d"/*-switchconv.ll 2>/dev/null | head -1)
  [ -n "$stage" ] || { echo "$p: no switchconv stage"; continue; }
  (echo 'target triple = "x86_64-unknown-linux-gnu"'; grep -v '^target triple' "$stage") > "$out/$p.mid.ll"
  "$b/llc" -O2 -relocation-model=pic -filetype=obj "$out/$p.mid.ll" -o "$out/$p.llc.o" 2>> "$out/$p.err" || { echo "$p: llc failed"; head -2 "$out/$p.err"; continue; }
  t() { size -A "$1" | awk '$1==".text"{print $2}'; }
  n() { objdump -d --no-show-raw-insn "$1" | grep -cE '^ +[0-9a-f]+:'; }
  printf '%-10s %7s %7s   %7s %7s\n' "$p" "$(t $out/$p.llc.o)" "$(t $out/$p.llrm.o)" "$(n $out/$p.llc.o)" "$(n $out/$p.llrm.o)"
done
echo "kept $out"
