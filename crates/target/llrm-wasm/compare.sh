#!/usr/bin/env bash
# Compares llrm-wasm-llc with llc-20 -O0 -fast-isel=false on each given .ll file, function by function.
#   crates/target/llrm-wasm/compare.sh FILE.ll...      (WRITE=1: store the oracle's function text as FILE.s, the goldens of tests/ir.rs)
# The oracle is the pin's llc (d1106deb71cc, WebAssembly only). LLC=llc-20 OURS=-no-stackify compares with LLVM 20,
# which does not run RegStackify at -O0.
set -uo pipefail
OURS=${OURS:-}
LLC=${LLC:-${LLRM_PIN_LLC:?set LLRM_PIN_LLC to the pin llc built with the WebAssembly target}}
BIN=${BIN:-$(cd "$(dirname "$0")/../../.." && pwd)/target/debug/llrm-wasm-llc}
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
# The function bodies of llc's output: from `.section .text.` to `# -- End function`, nothing else.
# A section directive left after the last function (the target features follow it) is not one.
# Comments and block labels are not compared: the structure is (`block`, `loop`, depths).
norm() { sed -E 's/[[:space:]]*#.*$//; s/[[:space:]]+$//; /^\.LBB[0-9_]+:$/d; /^[[:space:]]*$/d'; }
bodies() { awk '/^\t\.section\t\.text\./{buf=$0; p=1; next} p{buf=buf "\n" $0} /# -- End function/{if(p){print buf; p=0}}' "$1"; }
ok=0; bad=0
for f in "$@"; do
  "$LLC" -mtriple=wasm32 -O0 -fast-isel=false "$f" -o "$W/ref.s" 2>"$W/ref.err" || { echo "REF-FAIL $f: $(head -1 "$W/ref.err")"; bad=$((bad+1)); continue; }
  "$BIN" -S $OURS "$f" -o "$W/got.s" 2>"$W/got.err" || { echo "FAIL $f: $(head -1 "$W/got.err")"; bad=$((bad+1)); continue; }
  bodies "$W/ref.s" | norm > "$W/ref.txt"
  norm < "$W/got.s" > "$W/got.n"
  if [ -n "${WRITE:-}" ]; then cp "$W/ref.txt" "${f%.ll}.s"; fi
  if diff -q "$W/ref.txt" "$W/got.n" >/dev/null; then ok=$((ok+1)); else echo "DIFF $f"; diff "$W/ref.txt" "$W/got.n" | head -${LINES_SHOWN:-12}; bad=$((bad+1)); fi
done
echo "match $ok, differ $bad"
