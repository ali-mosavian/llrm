#!/usr/bin/env bash
# tools/sim65-insns.sh CC65_SRC OUT: a sim65 that also prints "N instructions" after its cycle count (-c), for the 6502 bench
# (LLRM_SIM65=OUT). CC65_SRC is a cc65 checkout; it is copied, not changed.
set -euo pipefail
src=$1
out=$2
work=$(mktemp -d)
cp -r "$src/src/sim65" "$work/sim65"
cp -r "$src/src/common" "$work/common"
python3 - "$work" <<'PY'
import sys
w = sys.argv[1]
p = f"{w}/sim65/6502.c"
s = open(p).read()
a = "unsigned ExecuteInsn (void)\n/* Execute one CPU instruction */\n{\n"
assert a in s
open(p, "w").write(s.replace(a, "unsigned long long InsnCount = 0;\n\n" + a + "    ++InsnCount;\n", 1))
p = f"{w}/sim65/error.c"
s = open(p).read()
a = '        fprintf (stdout, "%" PRIu64 " cycles\\n", Peripherals.Counter.ClockCycles);\n'
assert a in s
open(p, "w").write(s.replace(a, a + '        extern unsigned long long InsnCount;\n        fprintf (stdout, "%llu instructions\\n", InsnCount);\n', 1))
PY
gcc -O2 -w -fcommon -I"$work" -I"$work/common" -o "$out" "$work"/sim65/*.c "$work"/common/*.c
rm -rf "$work"
