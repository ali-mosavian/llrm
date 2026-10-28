#!/usr/bin/env bash
# Writes ids.txt for the pin's AArch64GenDAGISel.inc: `name value` for every OPC_*, ISD::, AArch64ISD::,
# MVT::, AArch64:: and TargetOpcode:: symbol its matcher table names, taken from the pin's own headers.
#
#   ids.sh OUT_DIR      (PIN names a checkout of the pin with a build in PIN/build)
set -euo pipefail
PIN=${PIN:-$HOME/work/other/llvm-pin}; OUT=$1; mkdir -p "$OUT"
B=$PIN/build; S=$PIN/llvm; A=$S/lib/Target/AArch64; INC=$B/lib/Target/AArch64/AArch64GenDAGISel.inc
sed -n '/MatcherTable\[\] = {/,/^  };/p' "$INC" | grep -oE '\b(ISD|AArch64ISD|MVT|AArch64|TargetOpcode|MCRegister)::[A-Za-z0-9_]+' | sort -u | sed 's/.*/P(&);/' > "$OUT/symbols.cpp"
grep -oE '\bOPC_[A-Za-z0-9_]+|\bOPFL_[A-Za-z0-9_]+' "$S/include/llvm/CodeGen/SelectionDAGISel.h" | grep -v OPFL_VariadicN | sort -u | sed 's/.*/P(&);/' > "$OUT/members.cpp"
# The OPC_* enums are protected members of SelectionDAGISel and its header will not compile alone: copy the text.
sed -n '/enum BuiltinOpcodes {/,/^  };/p;/^  enum {/,/^  };/p' "$S/include/llvm/CodeGen/SelectionDAGISel.h" > "$OUT/enums.cpp"
python3 -I - "$(dirname "$0")/ids.cpp.in" "$OUT" <<'PY'
import sys
src, work = sys.argv[1], sys.argv[2]
t = open(src).read()
for key, name in (("// @ENUMS@", "enums"), ("@SYMBOLS@", "symbols"), ("@MEMBERS@", "members")):
    t = t.replace(key, open(f"{work}/{name}.cpp").read())
open(work + "/ids.cpp", "w").write(t)
PY
g++ -std=c++17 -w -I"$S/include" -I"$B/include" -I"$A" -I"$B/lib/Target/AArch64" -I"$S/lib/Target" "$OUT/ids.cpp" -o "$OUT/ids" -Wl,-undefined,dynamic_lookup
"$OUT/ids" | sed 's/^SelectionDAGISel:://' > "$OUT/ids.txt"
