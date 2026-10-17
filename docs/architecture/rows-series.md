# Output rows against clang and gcc, by date

Each section is one comparison of llrm at two commits of `main`, on the same harness commit, in the vsgcc rows (`crates/target/llrm-x86-m32/vsgcc`): the 71 programs and the `x_` kernels at -O1, -O2, -O3 and -Os, `d_faces` executed on its seeded scene, and QCport's 65 modules by emitted bytes. The goal it checks: output never worse.

Clocks are the emulator's 486 clocks (unicorn), not wall time. The ratio of a row is llrm over the lower of gcc 13.4.0 and clang 20.1.8 on that row's counter; the geometric mean over the rows is read beside the worst row. gcc and clang objects are built once and shared by both sides, so only llrm differs. QCport has no gcc or clang row (Open Watcom headers): its modules are compared llrm with llrm.

Run: `tools/rows/run.sh main <llrm-c>`, then `tools/rows/run.sh base <llrm-c> main` for the other commit (its gcc and clang objects are the first run's), `vsgcc/dfaces/run.py --json` for each, `tools/rows/qcport.py`, `tools/rows/analyse.py`. The bench cells of every front end (BASIC, bq16, bq32, Nib, C), which vsgcc does not cover: `CARGO_TARGET_DIR=<dir whose release/ holds one commit's binaries> tools/bench/bench.py --opt O1 O2 O3 Os --no-judge --json X.json` for each side (about 25 minutes), then `tools/rows/bench_table.py BASE.json MAIN.json`.

## 2026-10-17, `3dc0221a6` against `90bce0774`

The 11 commits between them (#1892, #1942, #1922, #1905, #1926, #1957, #1927 and the docs) change no output: every llrm object of the 71 programs is byte-identical at all four levels (284 of 284), and every `d_faces` and QCport counter is equal. The base is the compiler it says: the two compile `d_faces` -O2 in 3,541 M and 3,332 M instructions.

Clocks, llrm over min(gcc, clang):

| level | geomean | worst row | rows above 1.00 |
|---|--:|---|--:|
| -O1 | 0.8875 | 2.01 (recfloat) | 27 |
| -O2 | 0.9044 | 1.80 (x_dct) | 31 |
| -O3 | 0.9021 | 1.91 (tile) | 27 |
| -Os | 0.9169 | 2.19 (lru) | 36 |

Code bytes, llrm over min(gcc, clang):

| level | geomean | worst row | rows above 1.00 |
|---|--:|---|--:|
| -O1 | 0.9435 | 2.68 (x_switch_dense) | 17 |
| -O2 | 1.0304 | 4.85 (rectwo) | 23 |
| -O3 | 1.2813 | 8.58 (queens) | 36 |
| -Os | 0.9471 | 1.77 (x_switch_ranges) | 19 |

Rows newly below or above 1.00: none, at any level. `d_faces` clocks over min(gcc, clang): -O1 1.020, -O2 1.005, -O3 1.023, -Os 1.008. QCport emitted bytes: 274,261 (-O1), 274,366 (-O2), 290,924 (-O3), 260,195 (-Os), unchanged.

The -O3 code geomean of 1.28 is the clone chain of `ipa-cp-clone` (gcc's own -O3 flag): queens -O3 is 8.6x the smaller compiler.

Bench cells, instructions and code bytes of the 848 cells (41 programs of BASIC, bq16, bq32 and Nib and 48 of C, at -O1, -O2, -O3 and -Os): no counter of any cell differs between the two commits, for any front end (bench.py `--no-judge`, the two sides' compilers differ in bytes).
