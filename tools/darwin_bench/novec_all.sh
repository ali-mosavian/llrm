#!/bin/bash
# novec_all.sh HEAD12 : (HEAD12 built by rerun.sh) novec.py over every bench program at O2 O1 O3 Os
P=$(cd "$(dirname "$0")" && pwd); export BENCH_REPO=${BENCH_REPO:-$(git -C "$P" rev-parse --show-toplevel)}
W=${W:-$(dirname "$BENCH_REPO")/llrm-bench}; export BENCH_WORK=$W/work LLRM_CC=$W/bin/$1/llrm-cc; : "${LLRM_PIN_CLANG:?}"; mkdir -p "$W/results"; cd "$BENCH_REPO"
# arm64 programs only: the DOS-specific ones (__huge, far pointers) do not compile for any arm64 compiler
PR=${PROGS:-$(for p in $(ls bench); do [ "$p" != grep ] && [ -f bench/$p/$p.c ] && [ -f bench/$p/$p.out ] && ! grep -q '__huge\|__far' bench/$p/$p.c && echo $p; done; echo grep)}
for L in O2 O1 O3 Os; do python3 "$P/novec.py" $L $PR > "$W/results/novec.$L.txt" 2>&1; done
