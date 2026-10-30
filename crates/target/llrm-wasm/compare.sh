#!/usr/bin/env bash
# Compares llrm-wasm-llc with llc-20 -O0 -fast-isel=false on each given .ll file, function by function.
#   crates/target/llrm-wasm/compare.sh FILE.ll...      (WRITE=1: store the oracle's function text as FILE.s, the goldens of tests/ir.rs)
# The oracle is the pin's llc (d1106deb71cc, WebAssembly only). LLC=llc-20 OURS=-no-stackify compares with LLVM 20,
# which does not run RegStackify at -O0.
set -uo pipefail
OURS=${OURS:-}
LLC=${LLC:-$HOME/scratch/llvm-build-wasm/bin/llc}
BIN=${BIN:-$(cd "$(dirname "$0")/../../.." && pwd)/target/debug/llrm-wasm-llc}
W=$(mktemp -d); trap 'rm -rf "$W"' EXIT
# The function bodies of llc's output: from `.section .text.` to `# -- End function`, nothing else.
# The section directive left at the end of the file, after the target features, is not one.
bodies() { awk '/^\t\.section\t\.text\./{p=1} p{print} /# -- End function/{p=0}' "$1" | sed '${/^\t\.section\t\.text\./d}'; }
ok=0; bad=0
for f in "$@"; do
  "$LLC" -mtriple=wasm32 -O0 -fast-isel=false "$f" -o "$W/ref.s" 2>"$W/ref.err" || { echo "REF-FAIL $f: $(head -1 "$W/ref.err")"; bad=$((bad+1)); continue; }
  "$BIN" -S $OURS "$f" -o "$W/got.s" 2>"$W/got.err" || { echo "FAIL $f: $(head -1 "$W/got.err")"; bad=$((bad+1)); continue; }
  bodies "$W/ref.s" > "$W/ref.txt"
  if [ -n "${WRITE:-}" ]; then cp "$W/ref.txt" "${f%.ll}.s"; fi
  if diff -q "$W/ref.txt" "$W/got.s" >/dev/null; then ok=$((ok+1)); else echo "DIFF $f"; diff "$W/ref.txt" "$W/got.s" | head -${LINES_SHOWN:-12}; bad=$((bad+1)); fi
done
echo "match $ok, differ $bad"
