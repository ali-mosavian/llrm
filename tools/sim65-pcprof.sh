#!/usr/bin/env bash
# tools/sim65-pcprof.sh CC65_SRC OUT: a sim65 that writes, at exit, the instructions and cycles executed at each
# address to the file named by $SIM65_PCPROF, one `ADDR COUNT CYCLES` line (hex address, decimal counts) per address
# that ran. Totals match `sim65 -c`. CC65_SRC is a cc65 checkout; it is copied, not changed. Use tools/6502/pcprof.py
# to key the lines by symbol and to diff two runs.
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
decl = (
    "#include <stdio.h>\n#include <stdlib.h>\n"
    "static unsigned long long ProfCount[65536], ProfCycles[65536];\n"
    "static void ProfDump (void)\n{\n"
    "    const char* Name = getenv (\"SIM65_PCPROF\");\n"
    "    FILE* F = Name ? fopen (Name, \"w\") : 0;\n"
    "    unsigned I;\n"
    "    if (F) {\n"
    "        for (I = 0; I < 65536; ++I) {\n"
    "            if (ProfCount[I]) {\n"
    "                fprintf (F, \"%04X %llu %llu\\n\", I, ProfCount[I], ProfCycles[I]);\n"
    "            }\n"
    "        }\n"
    "        fclose (F);\n"
    "    }\n}\n\n"
)
s = s.replace(a, decl + a + "    static int ProfInit;\n    unsigned ProfPC = Regs.PC;\n    if (!ProfInit) { ProfInit = 1; atexit (ProfDump); }\n", 1)
b = "    /* Count cycles */\n    TotalCycles += Cycles;\n"
assert b in s
s = s.replace(b, b + "    ProfCount[ProfPC & 0xFFFF] += 1;\n    ProfCycles[ProfPC & 0xFFFF] += Cycles;\n", 1)
open(p, "w").write(s)
PY
gcc -O2 -w -fcommon -I"$work" -I"$work/common" -o "$out" "$work"/sim65/*.c "$work"/common/*.c
rm -rf "$work"
