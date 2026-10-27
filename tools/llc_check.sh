#!/bin/sh
# tools/llc_check.sh [PROG...]: each tests/llc/PROG.c built by the pinned clang (reference) and by clang's front end
# alone -> llrm-llc -> gcc link, both run on this host; prints stdout/exit-status agreement and .text bytes.
# Columns: clang = a whole clang -O2 build (IR optimisation included, so it can fold the program away); pin = the
# pin's llc on the same IR llrm-llc gets; llrm = llrm-llc. Read llrm against pin for codegen, never against clang.
# Needs the pin (~/scratch/llvm-build, d1106deb71cc) and target/debug/llrm-llc (buildslot cargo build --bin llrm-llc).
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
clang=${LLRM_PIN_CLANG:-$HOME/scratch/llvm-build/bin/clang}
llc=${LLRM_LLC:-$root/target/debug/llrm-llc}
[ -n "${LLRM_NO_BUILD:-}" ] || (cd "$root" && buildslot cargo build --bin llrm-llc 2>&1 | grep -E "^error" -A8)
triple=${LLRM_TRIPLE:-x86_64-linux-gnu}
"$clang" --version | grep -q d1106deb71cc || { echo "llc_check: $clang is not the pin d1106deb71cc"; exit 2; }
"$clang" --version | head -1
out=$(mktemp -d)
progs=${*:-$(cd "$root/tests/llc" && ls *.c | sed 's/\.c$//')}
llcpin=${LLRM_PIN_LLC:-$(dirname "$clang")/llc}
printf '%-10s %-6s %8s %8s %8s   (.text bytes)\n' prog result clang pin llrm
bad=0
for p in $progs; do
  src=$root/tests/llc/$p.c
  "$clang" -target $triple -O2 -c "$src" -o "$out/$p.ref.o" && gcc "$out/$p.ref.o" -o "$out/$p.ref" -lm || { echo "$p: reference failed"; bad=1; continue; }
  "$clang" -target $triple -O2 -Xclang -disable-llvm-passes -S -emit-llvm "$src" -o "$out/$p.ll"
  if "$llc" -O2 "$out/$p.ll" -o "$out/$p.o" 2> "$out/$p.err" && gcc "$out/$p.o" -o "$out/$p.llrm" -lm 2>> "$out/$p.err"; then
    "$out/$p.ref" > "$out/$p.ref.out" 2>&1; rr=$?
    "$out/$p.llrm" > "$out/$p.llrm.out" 2>&1; lr=$?
    if [ $rr = $lr ] && cmp -s "$out/$p.ref.out" "$out/$p.llrm.out"; then r=ok; else r=DIFF; bad=1; fi
    a=$(size -A "$out/$p.ref.o" | awk '$1==".text"{print $2}'); b=$(size -A "$out/$p.o" | awk '$1==".text"{print $2}')
    "$llcpin" -O2 -filetype=obj "$out/$p.ll" -o "$out/$p.pin.o" 2>/dev/null && c=$(size -A "$out/$p.pin.o" | awk '$1==".text"{print $2}') || c=-
  else r=FAIL; bad=1; a=-; b=-; c=-; head -c 300 "$out/$p.err" | head -3
  fi
  printf '%-10s %-6s %8s %8s %8s\n' "$p" "$r" "$a" "$c" "$b"
done
exit $bad
