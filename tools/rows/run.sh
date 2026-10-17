#!/bin/bash
# rows/run.sh NAME LLRM_C [gccclang-from]: build + run the 71 vsgcc programs; llrm variants always, gcc/clang unless copied
set -e
# run.sh NAME LLRM_C [NAME_WITH_GCC_CLANG_RESULTS]: the 71 vsgcc programs; the first run (no third argument) also builds and runs gcc and clang
R=$(cd "$(dirname "$0")/../.." && pwd); H=$(echo $R/crates/target/*/vsgcc)
NAME=$1; export LLRM=$2; FROM=$3
export VSGCC_WORK=${ROWS_WORK:-$HOME/scratch/rows}/$NAME; mkdir -p $VSGCC_WORK
PROGS=$(python3 $H/programs.py)
# the stub's build is vsgcc/run.sh's own lines, not a copy of them
eval "$(grep -E '^(gcc -m32 -c|sha256sum) ' $H/run.sh)"
if [ -n "$FROM" ]; then
  mkdir -p $VSGCC_WORK/b $VSGCC_WORK/o; cp -a ${ROWS_WORK:-$HOME/scratch/rows}/$FROM/b/. $VSGCC_WORK/b/; cp -a ${ROWS_WORK:-$HOME/scratch/rows}/$FROM/references $VSGCC_WORK/references 2>/dev/null || true; cp -a ${ROWS_WORK:-$HOME/scratch/rows}/$FROM/abi $VSGCC_WORK/abi
  export VSGCC_ONLY=llrm
fi
echo $PROGS | tr ' ' '\n' | xargs -P 24 -I{} $H/build.sh {} 2>&1 | grep -E "FAIL" || true
VARS="llrmO1 llrm llrmO3 llrmOs"; [ -z "$FROM" ] && VARS="$VARS gccO1 gccO2 gccO3 gccOs clangO1 clangO2 clangO3 clangOs"
rm -rf $VSGCC_WORK/res; mkdir -p $VSGCC_WORK/res
for p in $PROGS; do echo $p; done | xargs -P 24 -I{} bash -c "for v in $VARS; do uv run --project $R/tools python $H/harness.py {} \$v | tail -1 >> $VSGCC_WORK/res/{}.jsonl; done"
cat $VSGCC_WORK/res/*.jsonl > $VSGCC_WORK/results.jsonl
cat $VSGCC_WORK/references 2>/dev/null || cat ${ROWS_WORK:-$HOME/scratch/rows}/$FROM/references 2>/dev/null || echo 'references: not recorded'
wc -l $VSGCC_WORK/results.jsonl
