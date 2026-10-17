# The allocator's decision procedure against RAGreedy's (plan step 3.3, re-planned)

TL;DR: the decisions are comparable (`d_faces` -O2: 837 assignments against LLVM's 815, 67 evictions against 78, 206 rounds against about 200 decisions) and a round costs 2.0 M against LLVM's 150 K. Of a round at its smallest (1-copy loop, 191 instructions, 490 K), 33% is recomputing a fact that did not change; the rest is the procedure: what llrm does for a popped value, row by row below, against what RAGreedy does. The prove-once items are about 12% of `regalloc` and are not the plan; the procedure is. Status: a re-plan, nothing built; the model of section 4 says a monotone-stage procedure on the existing LIR core could reach about 190 K per round under stated assumptions.

## 1. The accounting that closes

Chain-1: one function of one 24-statement loop, 191 instructions, 6 blocks, 76 rounds (49 spills, 50 split attempts, 33 carves, 104 evict searches), callgrind instructions. `_allocated` is 37.22 M inclusive: its direct callees sum to 36.84 M and its own code is 0.38 M. The eight largest callees are 75.5% of it; the other 24.5% is a tail of about 90 callees, none over 0.85 M, and the repeated patterns in it are rows of their own below (per-pop sorts, the per-round diff, hint walk). A flat self-instruction view double counts shared callees (mimalloc, memcpy) and is not used.

## 2. Step by step

M: chain-1 (per call) | `d_faces` -O2 on main plus #2010, #2021, #2027, #2032 (2,612 M).

| # | step per popped value | llrm | LLVM (RAGreedy) | what llrm lacks | class |
|--:|---|---|---|---|---|
| 1 | pop, bookkeeping | `BinaryHeap<Queued>` re-keyed from the weight at each push; `waiting` counts and a `popped` set; 0.09 M for 279 pushes | `PriorityQueue` of (stage, size, reg); `ExtraRegInfo` array indexed by vreg: stage and cascade; a stage only grows, so a range never returns to an earlier stage | the monotone stage (a requeue after a rewrite resets `Done` to `Assign` in 1.4% of rounds) | inherent, small |
| 2 | order the registers | `target::order` (1.37 M, 4,573 calls on `d_faces`) then hint votes, `Claims::by`, three sorts, and a `BTreeSet<RegId>` of word bases built per pop: 1.94 M + 0.59 M of sort for 273 pops (9.6 K each) | `AllocationOrder` over the class's cached order, hint first (`getRegAllocationHint`), no sets built | the order and the constant sets of a class built once per class | prove-once: 2.5 M |
| 3 | assign: first free register | `_free` 7.8 M for 2,396 calls (3.2 K); the interference query `LiveUnion::meeting` 8.5 M for 9,882 queries (860 instructions), `_clobbered` 5.0 M for 26,220 (190) | `checkInterference` 550 instructions per query, answered from a union per register with a version (`LiveRegMatrix`) | a version on each union so a query is not asked again (A5: "interference is a union per register with a version") | inherent per query, 1.6x LLVM's |
| 4 | evict | `_evict` 4.41 M for 104 searches (42 K): every register of the order meets the union, then each victim's `movable` search runs (2.51 M, 835 calls, 3 K each: is there another register for it), then `LiveRanges::weight` per victim (1.35 M, 1 K each) | `tryEvict`: one candidate per physreg from the interference cost (`canEvictInterferenceBasedOnCost`: weight below the evictee's, cascade number); no search for where the victim could go; weight read from the interval | weights stored in the interval at computation (CalcSpillWeights) and no mover test | `movable` is our procedure; `weight` prove-once |
| 5 | recolor | `Coloring::recolor` 3.32 M for 51 calls at 1 copy; 10.8 M for 119 on `d_faces`, 10.45 M of it in `crowded` | `tryRecoloring` exists in greedy but runs only for a range about to spill, depth-limited (`-greedy-recolor-max-depth`); not on every failed assign | a last-chance stage, not a stage of every value | our procedure |
| 6 | split | `Split::of`, `local`, `placed`, `per_block`, `pays` per attempt, then `carved_moving` (a new body): 2.56 M + 1.68 M for 50 attempts at 1 copy; `placed` 117.4 M before #1958 and 60.5 M after, carve 23.2 M, on `d_faces` | `SplitAnalysis` once, `SplitEditor` edits the one interval and makes new vregs; `SpillPlacement` on kept bundles (3.3 M for 28,095 activations); 3 `splitAroundRegion` on the diamond | the editor contract (a split edits through a log, A5/A6) | greedy-port |
| 7 | spill | `spilled_apart` 5.15 M for 49 spills at 1 copy (105 K); `materialized` 18.5, `planned` 11.3, `siblings_over` 9.0 on `d_faces`; `_tied` recomputed per spill (1.29 M) | `InlineSpiller::spill`: about 28 K, rematerialization decided on the def | per-instruction facts (tied operands) kept on the instruction | rewrite inherent, `_tied` prove-once |
| 8 | after the rewrite | `Facts::of` 9.04 M for 77 builds (119 K); `changes` 0.49; `HintWalk::update` 0.57 (3.96 on `d_faces`); `_next_value_following` 0.44 (8.0 on `d_faces`); overlap and requeue: the rewrite can take the register of a placed value (1.4% of rounds) | `LiveRangeEdit` hands the new vregs and the touched blocks to the next dequeue; the intervals of other vregs and their assignments are untouched; block frequency and bundles once per function | the edit record / `touched_since` (spec 2.3), per-update costs that depend on what changed | 6.0 M prove-once; the rest inherent |

## 3. What is recomputation and what is procedure

Prove-once (a fact that did not change, recomputed): facts update overhead 6.0 M, the per-pop order sets and sorts 2.5 M, weights 1.35 M, `_tied` 1.29 M, `crowded` 1.69 M: 12.8 M of the 37.2 M, 34% of the chain-1 total and about 12% of `regalloc` on `d_faces` (~50 M). Procedure: `movable` 2.51 M, the recolor search 1.63 M, the interference queries, the split path and the spill rewrite (about 10 M in all at 1 copy).

Correction of an earlier reading: greedy does have a recolor stage (`tryRecoloring`); it differs in being a last chance for a range that is about to spill and in being depth-limited, not in existing.

## 4. A model of the monotone-stage procedure at chain-1 scale

Assumptions, each to be tested by the slice (section 5), not known: the order and constant sets built once per class (row 2 to 0.3 K per pop); a stored weight; no `movable` search; recolor only for the range about to spill; facts updated for the touched values at 25 K a round (the O(touched) ideal at this size); the split and the spill as they are.

| per round, K instructions | today | modelled |
|---|--:|--:|
| facts (row 8) | 119 | 25 |
| spill rewrite (row 7: 49 spills / 76 rounds x 105 K) | 68 | 68 |
| evict (row 4: 104 / 76 x 42 K, less `movable` and `weight`) | 58 | 20 |
| recolor (row 5) | 44 | 5 |
| split and carve (row 6) | 56 | 56 |
| order sets and sorts (row 2) | 33 | 3 |
| the rest of the tail | 112 | 40 |
| total | 490 | 217 |

Under 250 K, so the kill ("cannot be modelled under 250 K") is not met; the margin is thin and rests on the three largest assumptions (the 25 K facts update, the 20 K evict and the 40 K tail). LLVM's 150 K per decision is not reached by this model, and the split and spill rows are untouched in it.

## 5. The slice before any build

One function family, chain-1 and the large `d_faces` function, behind the function's size as the walker's shortcut is: (a) hoist the constant sets and sorts of row 2 and store weights (pure, obvious; the first commit), (b) take the `movable` search out of `_evict` and replace it by greedy's rule (cost bound by weight and cascade), checking the allocation against today's with `identical.py` (it will change output: the rows rule of spec 4.3 applies), (c) make recolor a last-chance stage. Measure instructions per round on chain-1 and on the large function. Pass: chain-1 at most 250 K per round and `d_faces` `regalloc` down by 60 M or more. Kill: above 350 K per round, then the cost is in the split and spill rows and the allocator is closed as a constant factor away from LLVM's (recorded in spec 8).

## 6. Review questions

1. Is a monotone-stage allocator on the existing LIR core the right abstraction, or does the per-round cost come from the model (an immutable body rebuilt per edit) however the procedure is arranged?
2. Which rows of section 2 are not comparable with LLVM's, because the work is done elsewhere in LLVM (the register coalescer, the spill weights pass, the rewriter)?
3. What would a skilled engineer reading this in three years think was the wrong turn?
