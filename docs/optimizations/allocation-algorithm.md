# Allocation: where the time goes, and the algorithm to replace it

For contributors to the register allocator. Measured first; the design follows from the table, and nothing in it is built yet.

## Measured

`llrm-cc -O2 -m32`, instructions the compiler itself executes (`LLRM_DEBUG=time`), one function of each shape from `scaling.py` at N, 2N, 4N, 8N.

| Axis | N | compile Minstr | allocator Minstr | allocator share | rounds | allocations | allocator, ×per doubling |
|---|---|---|---|---|---|---|---|
| cells | 112 | 1,177 | 480 | 41% | 178 | 5 |  |
| cells | 224 | 3,001 | 1,476 | 49% | 424 | 5 | ×3.1 |
| cells | 448 | 8,040 | 4,395 | 55% | 874 | 5 | ×3.0 |
| cells | 896 | 24,611 | 14,885 | 60% | 1,800 | 5 | ×3.4 |
| branches | 100 | 2,635 | 641 | 24% | 72 | 5 |  |
| branches | 200 | 5,770 | 1,424 | 25% | 148 | 5 | ×2.2 |
| branches | 400 | 12,769 | 3,606 | 28% | 299 | 5 | ×2.5 |
| branches | 800 | 31,701 | 10,584 | 33% | 598 | 5 | ×2.9 |
| joins | 64 | 3,598 | 387 | 11% | 56 | 10 |  |
| joins | 128 | 8,545 | 1,245 | 15% | 148 | 10 | ×3.2 |
| joins | 256 | 22,001 | 4,224 | 19% | 388 | 10 | ×3.4 |
| joins | 512 | 65,672 | 14,306 | 22% | 908 | 10 | ×3.4 |
| live | 32 | 375 | 124 | 33% | 71 | 6 |  |
| live | 64 | 737 | 278 | 38% | 135 | 6 | ×2.2 |
| live | 128 | 1,565 | 672 | 43% | 263 | 6 | ×2.4 |
| live | 256 | 3,634 | 1,812 | 50% | 519 | 6 | ×2.7 |
| nest | 8 | 431 | 81 | 19% | 18 | 5 |  |
| nest | 16 | 1,088 | 204 | 19% | 42 | 5 | ×2.5 |
| nest | 32 | 3,678 | 1,274 | 35% | 216 | 6 | ×6.2 |
| nest | 64 | 9,933 | 2,835 | 29% | 405 | 6 | ×2.2 |
| chain | 16 | 161 | 34 | 21% | 27 | 4 |  |
| chain | 32 | 323 | 92 | 28% | 75 | 4 | ×2.7 |
| chain | 64 | 625 | 182 | 29% | 153 | 7 | ×2.0 |
| chain | 128 | 1,261 | 391 | 31% | 327 | 10 | ×2.1 |

On the axes that spill, the allocator is 25-60% of the compile and grows ×2.2-3.4 per doubling of N: the rounds (one per spill or split) grow ×1.9-2.1, and the cost of a round grows with the body. `cases` and `callers` (no spills) are linear.

Per round, `cells` 112 → 896, the steps that run once per round:

| Step (per round) | calls 112 → 896 | k instr per call 112 → 896 | per-call growth for 8× body | share of allocator | class |
|---|---|---|---|---|---|
| spill cleanup | 123 → 1,258 | 189 → 1,501 | ×8.0 | 12.7% | body |
| split carving | 59 → 601 | 415 → 2,093 | ×5.0 | 8.4% | body |
| regalloc recolor | 128 → 1,269 | 975 → 947 | ×1.0 | 8.1% | constant |
| after wanted | 178 → 1,800 | 86 → 654 | ×7.6 | 7.9% | body |
| regalloc split | 59 → 601 | 250 → 1,302 | ×5.2 | 5.3% | body |
| spill stable loads | 130 → 1,312 | 108 → 573 | ×5.3 | 5.1% | body |
| spill rewrite | 123 → 1,258 | 100 → 530 | ×5.3 | 4.5% | body |
| facts intervals | 183 → 1,805 | 105 → 302 | ×2.9 | 3.7% | touched |
| facts hints | 183 → 1,805 | 47 → 262 | ×5.6 | 3.2% | body |
| regalloc spill | 123 → 1,258 | 149 → 333 | ×2.2 | 2.8% | touched |
| split after liveness | 55 → 542 | 96 → 747 | ×7.8 | 2.7% | body |
| spill short updates | 123 → 1,258 | 39 → 261 | ×6.7 | 2.2% | body |
| split pays | 59 → 601 | 71 → 536 | ×7.6 | 2.2% | body |
| spill local updates | 123 → 1,258 | 29 → 253 | ×8.6 | 2.1% | body |
| facts slots | 183 → 1,805 | 33 → 175 | ×5.2 | 2.1% | body |
| split local | 59 → 601 | 135 → 477 | ×3.5 | 1.9% | touched |
| facts widths | 183 → 1,805 | 62 → 153 | ×2.5 | 1.9% | touched |
| classes scan | 205 → 1,827 | 66 → 122 | ×1.9 | 1.5% | touched |
| regalloc facts | 183 → 1,805 | 45 → 70 | ×1.6 | 0.8% | constant |
| facts sibling prices | 183 → 1,805 | 39 → 67 | ×1.7 | 0.8% | touched |

By class, as a share of the allocator at `cells` 896: **body** (per-call cost grows with the function, ×5-8 for ×8) 59%; touched (grows ×2-4) 13%; constant 11%. `d_faces -O1` (`D_DRAW_FACES`, 1,900 instructions): 4,400 of 9,600 Minstr in the allocator; the function is allocated 6 times (base, two trial shapes, two routes) in 838 rounds.

Steps whose own work grows at least N^1.9 (8× size, own work at the large size 150 Minstr or more): `spill cleanup` N^2.1, `after wanted` N^2.1, `spill short updates`, `spill local updates`, `split pays`, `split after liveness`, `split local` N^2.0-2.1, `facts hints` N^1.9-2.1 and `facts slots`; outside the allocator `lir coalesce` (joins, N^2.07, 10.4 G at 512), `mir sroa` (N^1.9), `ssa remakable` (cells, N^1.95), `mir unroll` (N^1.97), `mir lsr` (nest, N^2.18), `analysis counted` (nest, N^2.43).

## Three causes

1. **The function is allocated up to six times.** Base, two trial shapes (`Whole` without splitting, and the first of `Scoped`/`Retainable`/`Addressed`/`Unfolded` its spills admit), each on the spiller route and on the allocator alone. A trial's decisions share almost nothing with the base's: the first spill or split of a trial equals the base's in 0-4 of 119-189 rounds on `D_DRAW_FACES`, so it cannot start from the base's state.
2. **Each round re-derives state from the body.** After a spill or split `Facts::of` carries intervals, masks, widths and hints from the body before (touched values only), but the steps in the table's *body* class walk instructions or blocks of the whole function to find what changed: the postings are renumbered per block, `after wanted`/`hints` rescan, `split pays`/`after liveness` recompute liveness. LLVM's `LiveRangeEdit` and `SplitEditor` create the intervals of the new registers from the instructions they rewrote and leave the rest (`SlotIndexes` never renumber).
3. **Last-chance recolouring is attempted for every value no split rescued, and almost never succeeds** (`recolor`: 1,269 calls for 1,800 rounds at `cells` 896, ~0.95 M instructions each at every size, 8% of the allocator). `Coloring::BUDGET` records 178 successes in 19,600 sessions. It releases and retakes the holders in `LiveUnion` for each register tried before it asks whether any holder has another register. LLVM's `tryLastChanceRecoloring` has the same depth (5) and interference (8) limits, and is reached after eviction and splitting failed.

## What the trials decide differently

Over the 66 programs and 65 QCport modules at `-O2`: 711 of 1,332 allocations try a shape; a trial wins in 210 (30%) with median gain 2.9% (p90 10%, max 27%), `Whole` 19%, `Scoped` 24%, `Retainable` 26%, `Addressed` 9%. The wins are in small functions as much as large (<50 instructions: 32% of functions; ≥1,000: 56% of 9).

- `Whole` wins at the same rate (15-21%) whatever the number of splits the base made (1: 15%, 2-3: 19%, 4-9: 21%, 10+: 17%), and in 66 of 130 wins with the same number of spills as the base: the gain is where the spill code lands, not how much is spilled. Example `_savegame_scan@3` (36 instructions, -23%): the base splits values 2, 3 and 21 across blocks, then spills 3 and 2; without splitting, 21 and 2 are spilled together (siblings, one slot) and one value fewer is spilled.
- `Scoped` (loop-local copies of the base values a loop's spills lost, protected from spilling) is IRA's regional allocation done by hand: `_ent_place_models@3` (37 instructions, -20%) splits the same values the base did and spills them anyway; the trial keeps the loop's pieces and spills the rest of the range.
- `Retainable` keeps one base value in a register by its address-mode uses: a spill-weight decision (`_sc_lru_unlink@3`, -13%).

## Design

Each part names the technique it takes; the measured share it removes is the allocator share above.

1. **One allocation per function** (LLVM `RAGreedy` already has the shape: priority queue, stages `RS_Assign → Split → Split2 → Spill`, eviction with cascade numbers, `LiveIntervalUnion`). The trials become decisions inside the run, each taken where a live range is about to be split or spilled, by comparing the alternatives' cost there with block frequencies: spill, evict a lighter range (`tryEvict`), split around a loop with a protected piece (`tryRegionSplit` with `SpillPlacement`, and IRA's loop-region copies), keep the range whole (`RS_Spill`). Removes cause 1 for the shapes (3× of the allocator); the two routes (2×) stay, see below.
2. **Edits update the state they touch** (LLVM `LiveRangeEdit`, `SlotIndexes`, `LiveIntervals::repairIntervalsInRange`): intervals, hints, widths, masks, siblings' prices and the postings are keyed by stable slots and updated for the instructions an edit inserted or removed; no step walks a block it did not change. The *body* class (59%) becomes *touched*. The postings by slot (P1-P3 of the earlier note) are this part's data structure.
3. **A split is priced with its border copies after hinting** (`SplitEditor` + `RegisterCoalescer` hints; `calculateRegionSplitCost` counts the copies a piece's boundary needs unless both sides take one register). `Whole`'s wins with the same number of spills point here: the pieces' copies are priced as free and are not, once the pieces take different registers.
4. **Last-chance recolouring asks before it moves**: whether every holder of a register has another free register is answered without taking or releasing anything in `LiveUnion`; the search then runs only where it can succeed (LLVM `tryLastChanceRecoloring`, `LastChanceRecoloringMaxDepth`/`MaxInterference`). Same output; removes most of the 8%.
5. **Slots coloured once after allocation** (LLVM `StackSlotColoring`, IRA slot reuse): done in #1714.

The two routes (the spiller before the allocator, or the allocator alone) are not part of the allocator; they are chosen in `assemble.rs` (`directed`) and are 2× on every function whose spiller route is run to its end.

## What must be established before building

- **E1.** For each trial that wins, the set of values it treats differently from the base (held where the base spilled, spilled where it held), with their spill weight, uses, loop depth and whether the base had split them. A rule that predicts the win from the base run's own facts at the decision point is the single-run decision; none is known yet.
- **E2.** The quality gate: the single run's output cost over the corpus against today's best-of-six, per function, in the allocator's own cost and in bench rows; accepted when no bench row is worse and the corpus cost is no more than today's.
- **E3.** Per part, the table above with the part's steps: the *body* class steps measured again, expecting ×2-3 per 8× size.

Parts 2 and 4 keep today's output and can land first, each judged by the table. Part 1 changes output and lands behind a kill switch until E2 holds.
