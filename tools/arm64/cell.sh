#!/bin/bash
# cell.sh PROG LEVEL COMP -> one row for table.py: prog level comp bytes instrs metric value (ok|exception) flags
# flags = the canonical codegen flag set the column was built with (flagset.py, from $X.flags that build2.sh writes for ccn|clangn|gccn...); `unknown` for a column that records none, which table.py refuses.
# A/B two outputs with tools/arm64/cellcmp.py: it refuses rows of different columns (cc vs ccn) or unknown flags.
# Builds with build2.sh and counts the kernel range (count.py --kernel); the row says so.
# KERNEL-ONLY: a call into the runtime (memset, memcpy...) runs outside the range and is not counted, so a side that
# calls and a side that inlines the same fill are not comparable: rep_count read ours 1167 vs pin 926 (+26%) where the
# whole programs were 1311 vs 1204 (+9%). The libcalls the kernel makes go to `$X.libcalls` and to stderr; `libcalls.py
# OUT` lists the cells where the columns differ. Compare those on the whole program (count.py --whole).
# Env: UNICORN_PYTHON (python with unicorn), OUT, B, LLVM_BIN, CC, LLC as for build2.sh.
# A failed cell is a row, not a gap: `unmeasured:<reason>` (table.py lists it); never rerun silently.
H=$(cd "$(dirname "$0")" && pwd); P=$1; L=$2; C=$3
O=${OUT:-${TMPDIR:-/tmp}/arm64-bench-out}; mkdir -p "$O"; export OUT=$O
# the program's reference output: expect.py expected_path (NAME.arm64.out, NAME.ci.out, NAME.out)
expected_file() { python3 "$H/expect.py" --path "${B:-$H/../..}/bench" "$P"; }
unmeasured() { printf '%s\t%s\t%s\t\t\tkernel\tunmeasured:%s\t\n' "$P" "$L" "$C" "$1"; exit 0; }
applies=$(python3 "$H/expect.py" --applies "${B:-$H/../..}/bench/$P/$P.c") || unmeasured "$applies"  # `// targets:` that leaves out arm64 (textfill: dos)
bash "$H/build2.sh" "$P" "$L" "$C" >/dev/null 2>"$O/err.$P.$L.$C" || unmeasured "build failed, see $O/err.$P.$L.$C"
X=$O/$P.$L.$C
bytes=$(python3 -c "import sys;sys.path.insert(0,'$H');import matrix_kb;print(matrix_kb.kernel_bytes('$X.o'))" 2>>"$O/err.$P.$L.$C") || unmeasured "kernel bytes failed"
read -r value instrs _ < <("${UNICORN_PYTHON:-python3}" "$H/count.py" --kernel "$X.elf" 2>>"$O/err.$P.$L.$C")
[ -n "$instrs" ] || unmeasured "count failed, see $O/err.$P.$L.$C"
status=$(python3 "$H/expect.py" "$P" "$L" "$value" "$(expected_file)" 2>>"$O/err.$P.$L.$C") || unmeasured "value $value is not the expected one"  # a program that failed (-1) is no measurement
case $C in gccn|gccnnv) python3 "$H/flagset.py" --check-gcc "$X.flags" 2>>"$O/err.$P.$L.$C" || unmeasured "gcc flags differ from the neutral set, see $O/err.$P.$L.$C";; esac
libcalls=$("${V:-$HOME/scratch/llvm-build-aarch64/bin}/llvm-objdump" -r "$X.o" 2>/dev/null | awk '$2 ~ /CALL26|JUMP26/ {print $3}' | grep -E '^(memset|memcpy|memmove|memcmp|bcmp|strlen|strcpy|strcmp)$' | sort -u | tr '\n' ' ')
echo "$libcalls" > "$X.libcalls"
[ -z "$libcalls" ] || echo "cell.sh: $P $L $C: count is kernel-only; the runtime calls ${libcalls}are not in it" >&2
flags=$([ -f "$X.flags" ] && python3 "$H/flagset.py" "$X.flags" || echo unknown)
printf '%s\t%s\t%s\t%s\t%s\tkernel\t%s\t%s\n' "$P" "$L" "$C" "$bytes" "$instrs" "$status" "$flags"
