#!/bin/bash
# cell.sh PROG LEVEL COMP -> one row for table.py: prog level comp bytes instrs metric value (ok|exception) flags
# flags = the canonical codegen flag set the column was built with (flagset.py, from $X.flags that build2.sh writes for ccn|clangn|gccn...); `unknown` for a column that records none, which table.py refuses.
# Builds with build2.sh and counts the kernel range (count.py --kernel); the row says so.
# Env: UNICORN_PYTHON (python with unicorn), OUT, B, LLVM_BIN, CC, LLC as for build2.sh.
# A failed cell is a row, not a gap: `unmeasured:<reason>` (table.py lists it); never rerun silently.
H=$(cd "$(dirname "$0")" && pwd); P=$1; L=$2; C=$3
O=${OUT:-${TMPDIR:-/tmp}/arm64-bench-out}; mkdir -p "$O"; export OUT=$O
# the program's output on the part of the corpus the harness feeds it: NAME.ci.out where it has one (tools/bench/mkci.py), else NAME.out
expected_file() { local d="${B:-$H/../..}/bench/$P"; [ -f "$d/$P.ci.out" ] && echo "$d/$P.ci.out" || echo "$d/$P.out"; }
unmeasured() { printf '%s\t%s\t%s\t\t\tkernel\tunmeasured:%s\t\n' "$P" "$L" "$C" "$1"; exit 0; }
bash "$H/build2.sh" "$P" "$L" "$C" >/dev/null 2>"$O/err.$P.$L.$C" || unmeasured "build failed, see $O/err.$P.$L.$C"
X=$O/$P.$L.$C
bytes=$(python3 -c "import sys;sys.path.insert(0,'$H');import matrix_kb;print(matrix_kb.kernel_bytes('$X.o'))" 2>>"$O/err.$P.$L.$C") || unmeasured "kernel bytes failed"
read -r value instrs _ < <("${UNICORN_PYTHON:-python3}" "$H/count.py" --kernel "$X.elf" 2>>"$O/err.$P.$L.$C")
[ -n "$instrs" ] || unmeasured "count failed, see $O/err.$P.$L.$C"
status=$(python3 "$H/expect.py" "$P" "$L" "$value" "$(expected_file)" 2>>"$O/err.$P.$L.$C") || unmeasured "value $value is not the expected one"  # a program that failed (-1) is no measurement
case $C in gccn|gccnnv) python3 "$H/flagset.py" --check-gcc "$X.flags" 2>>"$O/err.$P.$L.$C" || unmeasured "gcc flags differ from the neutral set, see $O/err.$P.$L.$C";; esac
flags=$([ -f "$X.flags" ] && python3 "$H/flagset.py" "$X.flags" || echo unknown)
printf '%s\t%s\t%s\t%s\t%s\tkernel\t%s\t%s\n' "$P" "$L" "$C" "$bytes" "$instrs" "$status" "$flags"
