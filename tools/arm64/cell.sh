#!/bin/bash
# cell.sh PROG LEVEL COMP -> one row for table.py: prog level comp bytes instrs metric
# Builds with build2.sh and counts the kernel range (count.py --kernel); the row says so.
# Env: UNICORN_PYTHON (python with unicorn), OUT, B, LLVM_BIN, CC, LLC as for build2.sh.
H=$(cd "$(dirname "$0")" && pwd); P=$1; L=$2; C=$3
O=${OUT:-${TMPDIR:-/tmp}/arm64-bench-out}; export OUT=$O
bash "$H/build2.sh" "$P" "$L" "$C" >/dev/null 2>"$O/err.$P.$L.$C" || { echo "refused: $P $L $C" >&2; exit 1; }
X=$O/$P.$L.$C
bytes=$(python3 -c "import sys;sys.path.insert(0,'$H');import matrix_kb;print(matrix_kb.kernel_bytes('$X.o'))") || exit 1
read -r value instrs _ < <("${UNICORN_PYTHON:-python3}" "$H/count.py" --kernel "$X.elf")
python3 "$H/expect.py" "$value" "${B:-$H/../..}/bench/$P/$P.out" || exit 1  # a program that failed (-1) is no measurement
[ -n "$instrs" ] || { echo "unmeasured: $P $L $C" >&2; exit 1; }
printf '%s\t%s\t%s\t%s\t%s\tkernel\n' "$P" "$L" "$C" "$bytes" "$instrs"
