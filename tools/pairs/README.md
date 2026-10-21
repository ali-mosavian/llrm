# tools/pairs

Pair measurements: two builds of the compiler, the same rows, objects and runs on both, one report.

- `pair_build.sh`, `buildseq.sh`: one dist build per sha, cached.
- `rowdiff.py`, `objsweep.py`, `rundiff.py`, `runpinned.sh`: the rows, the objects and the tests/run outputs of the two arms. `runpinned.sh` runs tests/run from a tree checked out at the binaries' sha.
- `board.py`, `cbisect.py`, `firstbad.py`: the d_faces compile board, the compile-cost bisect, the first bad commit of a failure.
- `voidpair.py`: refuses a pair whose premise is broken.

A binary's md5 is not evidence that two builds are the same: embedded paths differ between worktrees. A claim that a change "cannot move" anything is made from the compared OBJECTS (`objsweep.py`); the md5 only proves that two arms are different binaries.

Quote every figure with its tool (callgrind Ir, `perf stat -e instructions:u`, a rows counter) and the arms line of the report.
