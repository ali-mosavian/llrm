#!/bin/bash
ulimit -v "${LLRM_MEMORY_LIMIT_KB:-8000000}"  # a runaway compile dies at 8 GB instead of taking the host (datetime.c grew to 50 GB)
# rows/row.sh PROG VARIANT LLRM_C: one row of the rows table, the JSON line harness.py prints (clocks, ins, mem, code ...). VARIANT: llrmO1 llrm (-O2) llrmO3 llrmOs.
#   tools/rows/row.sh x_life llrmOs /path/to/llrm-c      -> {"prog": "x_life", "variant": "llrmOs", ..., "clocks": N, "code": N}
# The program is built with that llrm-c (its objects alone; the gcc/clang objects are copied from the rows dir main24d, as run.sh does) and run under the 486 model.
set -e
R=$(cd "$(dirname "$0")/../.." && pwd); H=$(echo $R/crates/target/*/vsgcc)
PROG=$1; VARIANT=$2; export LLRM=$3
python3 "$R/tools/stamp.py" "$LLRM" >&2 || exit 3  # LLRM_EXPECT_SHA names the commit; LLRM_ALLOW_DIRTY=1 accepts a tree with changes
BASE=${ROWS_WORK:-$HOME/scratch/rows}
export VSGCC_WORK=$(mktemp -d "${TMPDIR:-/tmp}/row-XXXXXX"); trap 'rm -rf "$VSGCC_WORK"' EXIT
eval "$(grep -E '^(gcc -m32 -c|sha256sum) ' $H/run.sh)"
mkdir -p $VSGCC_WORK/b $VSGCC_WORK/o; cp -a $BASE/main24d/b/. $VSGCC_WORK/b/; cp -a $BASE/main24d/abi $VSGCC_WORK/abi
export VSGCC_ONLY=llrm
$H/build.sh $PROG 2>&1 | grep -E "FAIL" && exit 1
uv run -q --project $R/tools python $H/harness.py $PROG $VARIANT | tail -n 1
