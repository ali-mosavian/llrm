#!/bin/bash
# pair.sh NAME BASE_SHA NEW_SHA: dist builds of both shas (llrm-c, llrm-qb, llrm-nib, nibfront), then rows m32 (tools/rows/run.sh), tools/sizes.py m16 (C+qb+nib), objsweep and
# the tests/run outputs on a tree pinned to each sha; results in PAIR_SCRATCH/pair-NAME/ (rows-report.txt, sizes.txt, objsweep.txt, runs-*.txt, md5.txt, done).
# PAIR_WT: a worktree under PAIR_SCRATCH whose checkout builds the shas (one per concurrent pair); PAIR_SCRATCH defaults to ~/scratch. A script of this name in a shared
# scratch directory was overwritten by another session: this one lives in the repo and finds everything from its own path.
set -u
R=$(cd "$(dirname "$0")/../.." && pwd); S=${PAIR_SCRATCH:-$HOME/scratch}; N=$1; B=$2; W=$3; D=$S/pair-$N; mkdir -p $D
# BASE must be an ancestor of NEW, or the delta is not NEW's own commits (a pair once compared a base from another branch): refused unless --cross-branch is the 4th argument.
python3 $R/tools/pairs/ancestry.py $B $W ${4:-} > $D/header.txt || { cat $D/header.txt >&2; exit 2; }
cd "$S/${PAIR_WT:?PAIR_WT: a worktree to build the shas in}" || exit 2
for p in "base $B" "new $W"; do set -- $p
  T=$S/sbp-$2; ln -sfn $T $S/sb-$N-$1
  if [ ! -x $T/dist/llrm-c ] || [ ! -e $T/ok ]; then
    git checkout -q --detach $2 && bash $R/tools/pairs/pair_build.sh $S $2 > $D/build-$1.log 2>&1 && touch $T/ok || echo "build $1 failed" >> $D/status
  fi
  touch $T/dist/llrm-* $T/dist/nibfront
done
# A build is the sha the caller names, in a tree without changes: tools/stamp.py (a worktree moved during a build once gave a number for another commit).
python3 $R/tools/stamp.py --expect-sha $B $S/sb-$N-base/dist/llrm-c && python3 $R/tools/stamp.py --expect-sha $W $S/sb-$N-new/dist/llrm-c || { echo "stamp refused" >> $D/status; exit 3; }
{ md5sum $S/sb-$N-base/dist/llrm-c $S/sb-$N-new/dist/llrm-c | awk '{print substr($1,1,12), $2}'; } > $D/md5.txt
cd $R
# The rows and sizes steps run this tree's llrm-run, llrm-qb ...: a stale one stopped both ("stale binaries", pair csr, 2026-10-25).
cargo build --release --bins -q || { echo "bins build failed" >> $D/status; exit 3; }
export ROWS_WORK=$S/rows-$N; mkdir -p $ROWS_WORK; ln -sfn $S/rows/main24d $ROWS_WORK/main24d
nice -n 10 tools/rows/run.sh base $S/sb-$N-base/dist/llrm-c main24d > $ROWS_WORK/base.log 2>&1
nice -n 10 tools/rows/run.sh main $S/sb-$N-new/dist/llrm-c main24d > $ROWS_WORK/main.log 2>&1
for n in base main; do grep -E '"variant": ?"(gcc|clang)' $ROWS_WORK/main24d/results.jsonl >> $ROWS_WORK/$n/results.jsonl; done
bash tools/pairs/analyse_pair.sh rows-$N $S/sb-$N-base/dist $S/sb-$N-new/dist
{ cat $D/header.txt; cat $ROWS_WORK/report.txt; } > $D/rows-report.txt
python3 tools/sizes.py $S/sb-$N-base/dist $S/sb-$N-new/dist -O1 -O2 -O3 -Os > $D/sizes.txt 2>&1
python3 tools/pairs/objsweep.py $S/sb-$N-base/dist/llrm-c $S/sb-$N-new/dist/llrm-c --src $ROWS_WORK/main/b ${4:-} > $D/objsweep.txt 2>&1
for side in base new; do sha=$([ $side = base ] && echo $B || echo $W)
  bash tools/pairs/runpinned.sh $sha $S/sb-$N-$side/dist $D/runs-$side.txt
done
python3 tools/pairruns.py $D/runs-base.txt $D/runs-new.txt > $D/runs-diff.txt 2>&1
echo done > $D/done
