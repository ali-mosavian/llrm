# The split path against SplitKit, SplitAnalysis and SpillPlacement

TL;DR: `d_faces` -O2 spends 58.5 M in `Split::placed` for 489 candidate evaluations, 124 K a candidate against LLVM's at most 36 K for everything a candidate costs it. The count of candidates is not the gap (LLVM evaluates 837); the cost of a candidate is, and it is flat: no row is above 10.5 M. The levers that exist are small, exact, and together about 40 M (1.5%); they go in as one bundle.

Counts, same function (`D_DRAW_FACES`, 1,876 instructions; LLVM on `d_faces.c` with `llc -O2 -mtriple=i486-pc-linux-gnu`, callgrind; ours CI-mode build of main 7ac662405, same file):

| per run | LLVM | ours |
|---|--:|--:|
| split attempts | about 120 (`createFrom`) | 108 (74 carved) |
| candidate evaluations | 837 (`SpillPlacement::prepare`) | 489 (`Frame::grow`) |
| per attempt | 7.0 | 4.5 |
| constraint builds | 2,608 (`addConstraints`, 3.1 a candidate) | 489 `grow` + 497 `price` |
| `SpillPlacement` work | 4.0 M in all, 4.8 K a candidate | `Placement` `iterate` 3.8 + `activate` 3.3 + `update` 2.4 + `constrain` 1.1 + `new` 1.3 = 11.9 M, 24 K a candidate |
| everything a candidate costs | at most 30.3 M / 837 = 36 K (all of `allocatePhysRegs`) | `placed` 58.5 M / 489 = 120 K |

## 1. Rows

`placed`, 58.5 M, by owner and by what LLVM does at the same place.

| row | M | LLVM structure | here | probe / kill |
|---|--:|---|---|---|
| interference per candidate: `Blocked::at` to `Cursor::at` 10.5 (`interference` 5.2, `Destroyers::within` 1.9) + `Occupied::gather` 4.7 + `cursor` 3.9 | 19.1 | `InterferenceCache`: per (physreg, block), a revision number; refreshed only when the register's union changed | made again per split value, per register; nothing survives to the next split | probe: `gather` calls against distinct (register, holder set): the plan's 4.3.a. Kill: repeat ratio under 2 |
| `Frame::grow`, rows and bundles | 24.4 less the interference 10.5 = 13.9 (`iterate` 3.8, `activate` 3.3, `update` 2.4, `Vec` growth 3.2, the rest in row visits) | `growRegion`: activates the bundles around the use blocks, `SpillPlacement::iterate` on the active nodes | the same shape; 105 rows a `grow`, about 130 instructions a row visit after the interference is taken out | probe: `Vec` growth in `grow`/`link_bundles`: `with_capacity`; kill if under 1 M |
| `price` | 9.6 | `calculateRegionSplitCost`: the cost from the bundles' states | `copies` sorts the near blocks (`quicksort` 5.5 + `ipnsort` 3.0 = 8.5 M of it) and walks them | probe: replace the sort by a seen-table scan; kill if under 4 M saved |
| `held` | 6.6 | the bundle states are the answer; no per-row ranges | `Held::of` allocates zeroed per candidate (1.7), `ranges_in` per row | reuse one `Held` per attempt; kill if under 2 M |
| `Frame::new` | 5.5 | `SplitAnalysis::analyze`, once per attempt | once per attempt (108) | none: the shape LLVM has |
| `Placement::new`, `Facts::get_checked` | 1.3 + 2.0 | `SpillPlacement::prepare`, one call a candidate | once per attempt | none |

Not in `placed`: the carve (`carved_moving` 22.5 M, 74 carves), `live_among` after a split 10.0, `pays` 3.6, `split spread` 9.4, next-value 5.3. The carve from the allocator's postings (plan 3.3.d, about 22 M) is the one change in the path with a number above 20 M and is already in the queue.

## 2. What the counts say

- The count of candidates is not the gap: LLVM evaluates more (837). The cost of one is: 120 K here against at most 36 K there, 3.3 times at the bound, and 25 times if only `SpillPlacement` is compared (4.8 K there against 24 K).
- No row is above 19 M and none of `placed`'s pieces is a hash lookup: the position-table experiment (B') moved it by -0.03%. The cost is spread, which is why one change does not do it.
- The exact levers listed (interference cache about 9 M, the `price` sort about 7 M, `Vec` growth and `Held` reuse about 4 M) add to about 20 M with the carve's 22 M: 42 M, 1.6% of the compile, as a bundle (the rule: constant factors under 0.5% go in per area).

## 3. Order

1. Probes per row (above), each a counter and a test, in one scratch tree: report the five numbers.
2. Build the rows that pass as one PR, objects identical (`LLRM_CHECK_SPLIT`, `LLRM_CHECK_OCCUPIED`, the corpus).
3. The carve from postings stays with 3.3.d (shared with ssa-spill-m16: `_next_value_following` is theirs).

Not claimed: the 36 K bound is all of LLVM's `allocatePhysRegs` per candidate, so the true factor is at least 3.3; LLVM's `calcSplitConstraints` and `evaluateRegionSplit` are static functions with no symbols in the build, so their share is inside that figure.
