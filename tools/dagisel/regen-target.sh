#!/usr/bin/env bash
# Regenerates one target's DAG selector tables from the pin's tblgen.
#
#   tools/dagisel/regen-target.sh RECORD [OUT_DIR]   # RECORD: crates/target/llrm-<t>/dagisel.toml
#
# The record names the root .td, the ids headers and the CPUs (see dagisel-gen's target.rs).
# LLVM_SRC is a checkout of the pin, LLVM_BUILD a build of it with bin/llvm-tblgen and, for a
# target built in tree, its generated headers (lib/Target/<dir>). DAGISEL_GEN names a built
# llrm-dagisel-gen to use instead of cargo run.
set -euo pipefail

PIN=d1106deb71cc81016406a1917d0508d2df296266
LLVM_SRC=${LLVM_SRC:-$HOME/scratch/llvm-src}
LLVM_BUILD=${LLVM_BUILD:-$HOME/scratch/llvm-build-m68k}
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
REC=$(realpath "$1")
RECDIR=$(dirname "$REC")
OUT=${2:-$RECDIR/src/dagisel/generated}
if [ -z "${WORK:-}" ]; then WORK=$(mktemp -d); trap 'rm -rf "$WORK"' EXIT; fi
mkdir -p "$WORK" "$OUT"

[ "$(git -C "$LLVM_SRC" rev-parse HEAD)" = "$PIN" ] || { echo "LLVM_SRC is not at the pin $PIN" >&2; exit 1; }
S=$LLVM_SRC/llvm
field() { python3 -I -c 'import sys,tomllib; t=tomllib.load(open(sys.argv[1],"rb"))["target"]; v=t.get(sys.argv[2],""); print("\n".join(v) if isinstance(v,list) else v)' "$REC" "$1"; }

NS=$(field ns); ISD=$(field isd); DIR=$(field llvm_dir); TD=$(field td)
if [ -n "$DIR" ]; then
  T=$S/lib/Target/$DIR; INC=(-I"$T"); HDR=(-I"$T" -I"$LLVM_BUILD/lib/Target/$DIR" -I"$WORK")
else
  T=$RECDIR; INC=(-I"$T"); HDR=(-I"$T" -I"$WORK")
fi
TBLGEN=("$LLVM_BUILD/bin/llvm-tblgen" "${INC[@]}" -I"$LLVM_BUILD/include" -I"$S/include" -I"$S/lib/Target" -omit-comments)

"${TBLGEN[@]}" -gen-dag-isel "$T/$TD" -o "$WORK/GenDAGISel.inc"
"${TBLGEN[@]}" -gen-instr-info "$T/$TD" -o "$WORK/GenInstrInfo.inc"
# An in-crate .td has no build directory: its enums come from these (ids_includes names them).
"${TBLGEN[@]}" -gen-register-info "$T/$TD" -o "$WORK/GenRegisterInfo.inc"
python3 -I "$ROOT/tools/dagisel/instrinfo.py" "$WORK/GenInstrInfo.inc" "$WORK/instrinfo.txt" "$WORK/instr_symbols.txt" "${NS%::}"

# Only the tables name matcher symbols; the predicate code after them names types too.
OWN=$(field own_ids); [ -n "$OWN" ] && OWN=$RECDIR/$OWN || OWN=/dev/null
PREFIXES="ISD|MVT|TargetOpcode|${NS%::}|${ISD%::}"
sed -n '/MatcherTable\[\] = {/,/^  };/p' "$WORK/GenDAGISel.inc" |
  grep -oE "\\b($PREFIXES)::[A-Za-z0-9_]+" | sort -u | grep -vxFf <(cut -d' ' -f1 "$OWN") > "$WORK/symbols.txt" || true

grep -oE '\bOPC_[A-Za-z0-9_]+|\bOPFL_[A-Za-z0-9_]+' "$S/include/llvm/CodeGen/SelectionDAGISel.h" | grep -v OPFL_VariadicN | sort -u > "$WORK/members.txt"
# An own id is in no header: the program cannot name it, whoever lists it.
{ cat "$WORK/symbols.txt" "$WORK/instr_symbols.txt"; field extra_symbols; field registers | sed "/./s/^/$NS/"; } | grep . | sort -u | grep -vxFf <(cut -d' ' -f1 "$OWN") | sed 's/.*/P(&);/' > "$WORK/symbols.cpp"
sed -n '/enum BuiltinOpcodes {/,/^  };/p;/^  enum {/,/^  };/p' "$S/include/llvm/CodeGen/SelectionDAGISel.h" > "$WORK/enums.cpp"
sed 's/.*/P(&);/' "$WORK/members.txt" > "$WORK/members.cpp"
field ids_includes > "$WORK/includes.cpp"
python3 -I - "$ROOT/tools/dagisel/ids_target.cpp.in" "$WORK" <<'PY'
import sys
src, work = sys.argv[1], sys.argv[2]
t = open(src).read()
for key, f in (("// @ENUMS@", "enums"), ("// @INCLUDES@", "includes"), ("@SYMBOLS@", "symbols"), ("@MEMBERS@", "members")):
    t = t.replace(key, open(f"{work}/{f}.cpp").read())
open(work + "/ids.cpp", "w").write(t)
PY
g++ -std=c++17 -w -I"$S/include" -I"$LLVM_BUILD/include" "${HDR[@]}" "$WORK/ids.cpp" -o "$WORK/ids" -Wl,--unresolved-symbols=ignore-all
"$WORK/ids" | sed 's/^SelectionDAGISel:://' > "$WORK/ids.txt"
cat "$OWN" >> "$WORK/ids.txt"

if [ -n "${DAGISEL_GEN:-}" ]; then GEN=("$DAGISEL_GEN"); else GEN=(cargo run -q --release -p llrm-dagisel-gen --); fi
"${GEN[@]}" --target "$REC" --inc "$WORK/GenDAGISel.inc" --ids "$WORK/ids.txt" --pin "$PIN" --instrinfo "$WORK/instrinfo.txt" --out "$OUT"
