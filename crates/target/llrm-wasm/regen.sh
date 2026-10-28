#!/usr/bin/env bash
# Regenerates src/dagisel/generated. The pin build has no WebAssembly target, so the headers the ids need
# (WebAssembly*Gen*.inc) are made here with the pin's tblgen into a stand-in build directory.
set -euo pipefail

LLVM_SRC=${LLVM_SRC:-$HOME/scratch/llvm-src}
PIN_BUILD=${PIN_BUILD:-$HOME/scratch/llvm-build}
ROOT=$(cd "$(dirname "$0")/../../.." && pwd)
B=$(mktemp -d); trap 'rm -rf "$B"' EXIT
T=$LLVM_SRC/llvm/lib/Target/WebAssembly
G=$B/lib/Target/WebAssembly
mkdir -p "$B/bin" "$G"
ln -s "$PIN_BUILD/bin/llvm-tblgen" "$B/bin/llvm-tblgen"
ln -s "$PIN_BUILD/include" "$B/include"
for gen in "instr-info:InstrInfo" "register-info:RegisterInfo" "sd-node-info:SDNodeInfo" "subtarget:SubtargetInfo"; do
  "$B/bin/llvm-tblgen" "-gen-${gen%%:*}" -I"$LLVM_SRC/llvm/include" -I"$LLVM_SRC/llvm/lib/Target" -I"$T" \
    "$T/WebAssembly.td" -o "$G/WebAssemblyGen${gen##*:}.inc"
done
LLVM_BUILD=$B "$ROOT/tools/dagisel/regen-target.sh" "$ROOT/crates/target/llrm-wasm/dagisel.toml" "$@"
