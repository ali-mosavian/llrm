#!/bin/sh
# sim65_reach.py raised StopIteration on a module holding only `rc = ` lines (a split file with no code): bench_f64.py's f64 build failed for every op, no 6502 f64 row.
cd "$(dirname "$0")" || exit 2; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT
printf 'rc0 = 0\nrc1 = 1\n' > $T/empty.s
printf '.export main\nmain:\n  rts\n' > $T/main.s
if python3 sim65_reach.py $T/out main $T/main.s $T/empty.s 2>&1 | grep -q StopIteration; then echo "FAIL: StopIteration on a module with no code"; exit 1; fi
echo "sim65_reach_empty ok"
