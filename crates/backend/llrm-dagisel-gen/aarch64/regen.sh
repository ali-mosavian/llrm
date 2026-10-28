#!/usr/bin/env bash
# Regenerates the checked-in AArch64 DAG selector table from the pin's own tblgen.
#
#   crates/backend/llrm-dagisel-gen/aarch64/regen.sh [OUT_DIR]   # default: crates/backend/llrm-dagisel/src/generated
#
# PIN is a checkout of the pin with a build that has bin/llvm-tblgen (default ~/work/other/llvm-pin).
# DAGISEL_GEN names a built llrm-dagisel-gen to use instead of cargo run.
set -euo pipefail

PINSHA=d1106deb71cc81016406a1917d0508d2df296266
PIN=${PIN:-$HOME/work/other/llvm-pin}
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../../.." && pwd)
OUT=${1:-$ROOT/crates/backend/llrm-dagisel/src/generated}
if [ -z "${WORK:-}" ]; then WORK=$(mktemp -d); trap 'rm -rf "$WORK"' EXIT; fi
mkdir -p "$WORK"

[ "$(git -C "$PIN" rev-parse HEAD)" = "$PINSHA" ] || { echo "PIN is not at the pin $PINSHA" >&2; exit 1; }
S=$PIN/llvm; A=$S/lib/Target/AArch64; B=$PIN/build
TBLGEN=("$B/bin/llvm-tblgen" -I"$A" -I"$B/include" -I"$S/include" -I"$S/lib/Target")

"${TBLGEN[@]}" -gen-dag-isel -omit-comments "$A/AArch64.td" -o "$WORK/AArch64GenDAGISel.inc"
"${TBLGEN[@]}" -gen-instr-info "$A/AArch64.td" -o "$WORK/AArch64GenInstrInfo.inc"
python3 -I "$HERE/instrinfo.py" "$WORK/AArch64GenInstrInfo.inc" "$WORK/instrinfo.txt" "$WORK/instr_symbols.txt" AArch64
PIN=$PIN "$HERE/ids.sh" "$WORK" "$WORK/instr_symbols.txt"

if [ -n "${DAGISEL_GEN:-}" ]; then GEN=("$DAGISEL_GEN"); else GEN=(cargo run -q --release -p llrm-dagisel-gen --); fi
"${GEN[@]}" --target aarch64 --inc "$WORK/AArch64GenDAGISel.inc" --ids "$WORK/ids.txt" --pin "$PINSHA" \
  --features "$HERE/../features_aarch64.toml" --cpu apple-m1 --instrinfo "$WORK/instrinfo.txt" --out "$OUT"
