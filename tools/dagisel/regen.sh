#!/usr/bin/env bash
# Regenerates the checked-in DAG selector tables from the pin's own tblgen.
#
#   tools/dagisel/regen.sh [OUT_DIR]   # default: crates/backend/llrm-dagisel/src/generated
#
# DAGISEL_GEN names a built llrm-dagisel-gen to use instead of cargo run.
# LLVM_SRC is a checkout of the pin and LLVM_BUILD a build of it that has bin/llvm-tblgen.
set -euo pipefail

PIN=d1106deb71cc81016406a1917d0508d2df296266
LLVM_SRC=${LLVM_SRC:-$HOME/work/personal/llvm-project}
LLVM_BUILD=${LLVM_BUILD:-$HOME/scratch/llvm-build-assert}
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${1:-$ROOT/crates/backend/llrm-dagisel/src/generated}
if [ -z "${WORK:-}" ]; then WORK=$(mktemp -d); trap 'rm -rf "$WORK"' EXIT; fi
mkdir -p "$WORK"

[ "$(git -C "$LLVM_SRC" rev-parse HEAD)" = "$PIN" ] || { echo "LLVM_SRC is not at the pin $PIN" >&2; exit 1; }
S=$LLVM_SRC/llvm
X=$S/lib/Target/X86

"$LLVM_BUILD/bin/llvm-tblgen" -gen-dag-isel -I"$X" -I"$LLVM_BUILD/include" -I"$S/include" -I"$S/lib/Target" \
  -omit-comments "$X/X86.td" -o "$WORK/X86GenDAGISel.inc"

# Our own records (m16.td) on top of the pin's X86.td: the same tblgen, a second table.
"$LLVM_BUILD/bin/llvm-tblgen" -gen-dag-isel -I"$X" -I"$LLVM_BUILD/include" -I"$S/include" -I"$S/lib/Target" -I"$ROOT/tools/dagisel" \
  -omit-comments "$ROOT/tools/dagisel/m16.td" -o "$WORK/X86GenDAGISel_m16.inc"

# Only the tables name matcher symbols; the predicate code after them names types too.
sed -n '/MatcherTable\[\] = {/,/^  };/p' "$WORK/X86GenDAGISel.inc" "$WORK/X86GenDAGISel_m16.inc" |
  grep -oE '\b(ISD|X86ISD|MVT|X86|TargetOpcode|MCRegister)::[A-Za-z0-9_]+' | sort -u |
  grep -vxFf <(cut -d' ' -f1 "$ROOT/tools/dagisel/own_ids.txt") > "$WORK/symbols.txt"
# The pin's MCInstrDesc rows, implicit register lists and operand infos, as one line per
# instruction: `opcode numops numdefs size flags uses defs ops` (see tools/dagisel/instrinfo.py).
"$LLVM_BUILD/bin/llvm-tblgen" -gen-instr-info -I"$X" -I"$LLVM_BUILD/include" -I"$S/include" -I"$S/lib/Target" -I"$ROOT/tools/dagisel" \
  -omit-comments "$ROOT/tools/dagisel/m16.td" -o "$WORK/X86GenInstrInfo.inc"
python3 -I "$ROOT/tools/dagisel/instrinfo.py" "$WORK/X86GenInstrInfo.inc" "$WORK/instrinfo.txt" "$WORK/instr_symbols.txt"

grep -oE '\bOPC_[A-Za-z0-9_]+|\bOPFL_[A-Za-z0-9_]+' "$S/include/llvm/CodeGen/SelectionDAGISel.h" | grep -v OPFL_VariadicN | sort -u > "$WORK/members.txt"
extras=$(cat "$ROOT/tools/dagisel/extra_symbols.txt")
{ cat "$WORK/symbols.txt" "$WORK/instr_symbols.txt"; echo "$extras"; } | grep . | sort -u | sed 's/.*/P(&);/' > "$WORK/symbols.cpp"
# The OPC_* and OPFL_* enums are protected members of SelectionDAGISel, and its header will not compile alone: copy the text.
sed -n '/enum BuiltinOpcodes {/,/^  };/p;/^  enum {/,/^  };/p' "$S/include/llvm/CodeGen/SelectionDAGISel.h" > "$WORK/enums.cpp"
sed 's/.*/P(&);/' "$WORK/members.txt" > "$WORK/members.cpp"
python3 -I - "$ROOT/tools/dagisel/ids.cpp.in" "$WORK" <<'PY'
import sys
src, work = sys.argv[1], sys.argv[2]
t = open(src).read()
t = t.replace("// @ENUMS@", open(work + "/enums.cpp").read()).replace("@SYMBOLS@", open(work + "/symbols.cpp").read()).replace("@MEMBERS@", open(work + "/members.cpp").read())
open(work + "/ids.cpp", "w").write(t)
PY
if [ "$(uname)" = Darwin ]; then UNRESOLVED=(-Wl,-undefined,dynamic_lookup); else UNRESOLVED=(-Wl,--unresolved-symbols=ignore-all); fi
g++ -std=c++17 -w -I"$S/include" -I"$LLVM_BUILD/include" -I"$X" -I"$LLVM_BUILD/lib/Target/X86" \
  "$WORK/ids.cpp" -o "$WORK/ids" "${UNRESOLVED[@]}"
"$WORK/ids" | sed 's/^SelectionDAGISel:://' > "$WORK/ids.txt"
# Numbers for the X86ISD nodes m16.td defines; the pin's headers do not have them.
cat "$ROOT/tools/dagisel/own_ids.txt" >> "$WORK/ids.txt"

mkdir -p "$OUT"
if [ -n "${DAGISEL_GEN:-}" ]; then GEN=("$DAGISEL_GEN"); else GEN=(cargo run -q --release -p llrm-dagisel-gen --); fi
"${GEN[@]}" --inc "$WORK/X86GenDAGISel.inc" --ids "$WORK/ids.txt" --pin "$PIN" --inc-m16 "$WORK/X86GenDAGISel_m16.inc" --out "$OUT" --extras "$ROOT/tools/dagisel/extra_symbols.txt" --instrinfo "$WORK/instrinfo.txt"
