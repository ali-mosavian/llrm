#!/usr/bin/env bash
# vs.sh DIST LEVEL FILE...: code-section bytes of each FILE on wasm32, ours (DIST/llrm-cc) against the pin (clang -ffreestanding), with
# the file's own flags (cell_flags.py). Run from the repo root; LLRM_CLANG_WASM names the pin's clang.
#   FILE -LEVEL ours N pin M     both compiled
#   FILE -LEVEL ours FAILED(why) pin M    a side that failed says so; the other side still runs
#   FILE -LEVEL n/a (reason)     the corpus says the program is not for wasm32 (`// targets:`, corpus_targets.py): no cell
set -u
D=$1; L=${2#-}; shift 2
here=$(cd "$(dirname "$0")" && pwd); P=${LLRM_CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang}
w=$(mktemp -d); trap 'rm -rf "$w"' EXIT
code() { wasm-objdump -h "$1" | sed -n 's/.*Code.*size=0x\([0-9a-f]*\).*/\1/p' | while read -r h; do echo $((16#$h)); done; }
side() { # NAME COMMAND...: the code size, or FAILED(first error line)
  local name=$1; shift
  if "$@" -c "$f" -o "$w/$name.o" 2>"$w/$name.err"; then code "$w/$name.o"; else echo "FAILED($(grep -m1 -iE 'error|not found' "$w/$name.err" | head -c 80))"; fi
}
for f in "$@"; do
  b=$(basename "$f")
  if reason=$(python3 "$here/../corpus_targets.py" check wasm32 "$f" 2>/dev/null); then :; elif [ $? -eq 3 ]; then echo "$b -$L n/a ($reason)"; continue; fi
  pf=$(python3 "$here/cell_flags.py" "$f")
  inc="-I bench -I runtime/shared/posix -I tests/run/c"
  o=$(LLRM_CC_TARGET=wasm32 side o "$D/llrm-cc" $pf "-$L" $inc)
  p=$(side p "$P" --target=wasm32 -ffreestanding $pf "-$L" $inc)
  echo "$b -$L ours $o pin $p"
done
