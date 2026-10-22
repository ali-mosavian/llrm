#!/bin/bash
# pair.sh NAME BASE_SHA NEW_SHA: dist builds of both shas (llrm-c, llrm-qb, llrm-nib, nibfront), then rows m32 (tools/rows/run.sh), tools/sizes.py m16 (C+qb+nib), objsweep and
# the tests/run outputs on a tree pinned to each sha; results in PAIR_SCRATCH/pair-NAME/ (rows-report.txt, sizes.txt, objsweep.txt, runs-*.txt, md5.txt, done).
# Each sha is built in a worktree of its own (tools/pairs/build_sha.sh), never in a directory two builds share; PAIR_SCRATCH defaults to ~/scratch. A script of this name in a shared
# scratch directory was overwritten by another session: this one lives in the repo and finds everything from its own path.
set -u
R=$(cd "$(dirname "$0")/../.." && pwd); S=${PAIR_SCRATCH:-$HOME/scratch}; N=$1; B=$2; W=$3; D=$S/pair-$N; mkdir -p $D
for p in "base $B" "new $W"; do set -- $p
  ln -sfn $S/sbp-$2 $S/sb-$N-$1
  bash $R/tools/pairs/build_sha.sh $R $S $2 > $D/build-$1.log 2>&1 || echo "build $1 failed" >> $D/status
  touch $S/sbp-$2/dist/llrm-* $S/sbp-$2/dist/nibfront
done
# A build is the sha the caller names, in a tree without changes: tools/stamp.py (a worktree moved during a build once gave a number for another commit).
python3 $R/tools/stamp.py --expect-sha $B $S/sb-$N-base/dist/llrm-c && python3 $R/tools/stamp.py --expect-sha $W $S/sb-$N-new/dist/llrm-c || { echo "stamp refused" >> $D/status; exit 3; }
{ md5sum $S/sb-$N-base/dist/llrm-c $S/sb-$N-new/dist/llrm-c | awk '{print substr($1,1,12), $2}'; } > $D/md5.txt
cd $R
export ROWS_WORK=$S/rows-$N; mkdir -p $ROWS_WORK; ln -sfn $S/rows/main24d $ROWS_WORK/main24d
nice -n 10 tools/rows/run.sh base $S/sb-$N-base/dist/llrm-c main24d > $ROWS_WORK/base.log 2>&1
nice -n 10 tools/rows/run.sh main $S/sb-$N-new/dist/llrm-c main24d > $ROWS_WORK/main.log 2>&1
for n in base main; do grep -E '"variant": ?"(gcc|clang)' $ROWS_WORK/main24d/results.jsonl >> $ROWS_WORK/$n/results.jsonl; done
bash tools/pairs/analyse_pair.sh rows-$N $S/sb-$N-base/dist $S/sb-$N-new/dist
cp $ROWS_WORK/report.txt $D/rows-report.txt
python3 tools/sizes.py $S/sb-$N-base/dist $S/sb-$N-new/dist -O1 -O2 -O3 -Os > $D/sizes.txt 2>&1
python3 tools/pairs/objsweep.py $S/sb-$N-base/dist/llrm-c $S/sb-$N-new/dist/llrm-c --src $ROWS_WORK/main/b > $D/objsweep.txt 2>&1
for side in base new; do sha=$([ $side = base ] && echo $B || echo $W)
  bash tools/pairs/runpinned.sh $sha $S/sb-$N-$side/dist $D/runs-$side.txt
done
python3 tools/pairruns.py $D/runs-base.txt $D/runs-new.txt > $D/runs-diff.txt 2>&1
echo done > $D/done
