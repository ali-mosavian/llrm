---
architecture-index: output rows by date
architecture-order: 420
---
# Output rows against clang and gcc, by date

> **Void as a reference.** Every ratio and count in this file reads `rows/main` and `rows/base`, built with the Ubuntu clang 20.1.8; `rowcheck.compiler_premise` ([#2226]) refuses 284 of 284 of those objects. The pinned rows are `main24` to `main24d` (clang 24.0.0git `d1106deb71cc`). The llrm-against-llrm statements (objects byte-identical, ablation deltas) are unaffected; every figure against min(gcc, clang) is to be redone on `main24d`, except the ablation section, which is re-taken there.

Each section is one comparison of llrm at two commits of `main`, on the same harness commit, in the vsgcc rows (`crates/target/llrm-x86-m32/vsgcc`): the 71 programs and the `x_` kernels at -O1, -O2, -O3 and -Os, `d_faces` executed on its seeded scene, and QCport's 65 modules by emitted bytes. The goal it checks: output never worse.

Clocks are the emulator's 486 clocks (unicorn), not wall time. The ratio of a row is llrm over the lower of gcc 13.4.0 and clang 20.1.8 on that row's counter; the geometric mean over the rows is read beside the worst row. gcc and clang objects are built once and shared by both sides, so only llrm differs. QCport has no gcc or clang row (Open Watcom headers): its modules are compared llrm with llrm.

Run: `tools/rows/run.sh main <llrm-cc>`, then `tools/rows/run.sh base <llrm-cc> main` for the other commit (its gcc and clang objects are the first run's), `vsgcc/dfaces/run.py --json` for each, `tools/rows/qcport.py`, `tools/rows/analyse.py`. The bench cells of every front end (BASIC, bq16, bq32, Nib, C), which vsgcc does not cover: `CARGO_TARGET_DIR=<dir whose release/ holds one commit's binaries> tools/bench/bench.py --opt O1 O2 O3 Os --no-judge --json X.json` for each side (about 25 minutes), then `tools/rows/bench_table.py BASE.json MAIN.json`.

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

## 2026-10-17, what the costly analyses buy

**Re-taken against the pinned references.** The first version of this table read its output column against `rows/main`, built with the Ubuntu clang 20.1.8 (void, [#2226]). The output column below is re-taken against `rows/main24d` (clang 24.0.0git `d1106deb71cc` and gcc 13.4.0, `-fno-builtin-strlen`; llvm-order-poc, 0 not-ok or no-clocks rows in all seven ablation rows directories of 284 and in `main24d` of 852). The llrm columns are the same llrm commit `21f5c9fc8` with and without `LLRM_ABLATE`, so ablated against base is reference independent; the compile costs and the objects-differ facts did not change. The conclusion stands. Each cell reads "before to after" of the ablation, against `main24d`.

An ablation, not a landing: each analysis answers a coarse default (`LLRM_ABLATE=name`, [`ablate-analyses.patch`](ablate-analyses.patch), applies to `21f5c9fc8`, local only) and the same binary without the switch is the base. The defaults: pointer-values and bounded `Err`; float-facts empty; through-memory and annotated empty maps; exposed-frames every alloca exposed. A bare `Err` is not a coarse answer for the others (sroa and promote `expect` it), and full `points-to` has none that is sound (an empty answer says nothing escaped), so it is not ablated.

Compile cost in M instructions, none: `d_faces` -O2 3,271, -Os 3,335, queens -O3 1,670.

| family | d_faces -O2 | d_faces -Os | queens -O3 | output |
|---|--:|--:|--:|---|
| pointer-values | +1.4% | +1.3% | +0.2% | 0 of 1,323 objects differ; every row and `d_faces` counter equal |
| bounded | +7.8% | +6.4% | +11% | rows within 0.2%; no row crosses 1.00; clocks rows above min equal before and after (30 / 32 / 28 / 39) |
| float-facts | -1.3% | +2.9% | 0 | code +0.2% at -O2; fpbench -O3 clocks 0.985 to 1.037 (crosses above 1.00); code rows above min 39 to 40 at -O3, 20 to 21 at -Os |
| exposed-frames | -12.8% | -14.9% | -94% | clocks geomean -O1 0.8979 to 0.9164, -O2 0.9277 to 0.9477, -O3 0.9088 to 0.9355; rows above min at -O2 32 to 35; queens -O3 0.999 to 1.988 |
| through-memory | -1.0% | +0.1% | -88% | -O3 geomean 0.9088 to 0.9874, rows above min 28 to 31; matmul -O3 0.987 to 8.178, scroll -O3 0.997 to 3.152 |
| annotated | -50% | -51% | -96% | clocks geomean -O2 0.9277 to 1.8543; rows above min 32 to 69 at -O2, 28 to 68 at -O3, 39 to 71 at -Os; scroll -O2 11.3x |

No family is idle precision. Those whose removal costs no output save nothing; those that save compile time lose output, and the saving is optimizations that no longer happen, not cost removed. Keep them exact and make computing them cheap. `identical.py --programs-only` lists only the first 20 differences, so ">=20" objects differing is a floor; QCport was not run.

## 2026-10-17, `9bce914f38ed` against `3dc0221a6` (the first baseline)

21 commits since `5344082af` (#1961, #1956, #1959, #1970, #1975 and their neighbours) change no output. Same harness commit, shared gcc and clang objects; the head and the baseline compilers differ (`d_faces` -O2 compile 2,965 M against 3,541 M).

| instrument | result |
|---|---|
| vsgcc, 71 programs x 4 levels | all 284 llrm objects byte-identical to the baseline; clocks geomean over min(gcc, clang) 0.8875 / 0.9044 / 0.9021 / 0.9169 at -O1 / -O2 / -O3 / -Os, worst rows as before (recfloat 2.01, x_dct 1.80, tile 1.91, lru 2.19); no row crosses 1.00 |
| `d_faces` executed | every counter equal; clocks over min(gcc, clang) 1.020 / 1.005 / 1.023 / 1.008 |
| QCport, 65 modules | emitted bytes equal (274,261 / 274,366 / 290,924 / 260,195) |
| bench cells, all front ends | 848 cells, no counter differs |


[#2226]: https://github.com/ali-mosavian/llrm/pull/2226
