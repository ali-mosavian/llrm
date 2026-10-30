---
architecture-index: the allocator's decision procedure against RAGreedy's
architecture-order: 190
---
# The allocator's decision procedure against RAGreedy's (plan step 3.3, re-planned)

TL;DR: on `d_faces` -O2 the decisions are comparable (837 assignments against LLVM's 815, 67 evictions against 78) and a round costs 2.0 M against LLVM's 150 K per decision; on a controlled loop (chain-1) llrm emits about twice the stack operands clang does (83 against 34; 3,172 against 1,535 at 32 copies), so there the decision count is not known to be comparable. Of a round at its smallest (490 K), 34% is recomputing a fact that did not change and 47% is the round itself (facts, the diff, the body rebuilt, the tail). Reshaping the procedure (rows 2, 4, 5) can save at most 107 K of the 490 K, leaving about 380 K, above the 250 K kill: the kill is MET. The per-round cost comes from the model (an immutable body rebuilt per edit, facts derived from a diff), which is invariant A5 of the architecture page, not from the order in which the procedure tries things. Status: a re-plan, nothing built; the slice of section 5 is reduced to its pure part.

> **Reference note.** The LLVM allocator figures on this page (greedy 39.5 M, 815 assignments, 78 evictions, 122, 13/22/33 K per decision) were taken from the distro `llc` 20.1.8, are not reproduced on the pinned or the distro `llc`, and are being re-taken on the pinned 24.0.0git `d1106deb71cc` by regalloc-core-poc. Do not quote them until its old-versus-new lands.

## 1. The accounting that closes

Chain-1: one function of one 24-statement loop, 191 instructions, 6 blocks, 76 rounds (49 spills, 50 split attempts, 33 carves, 104 evict searches), callgrind instructions. `_allocated` is 37.22 M inclusive: its direct callees sum to 36.84 M and its own code is 0.38 M. The eight largest callees are 75.5% of it; the other 24.5% is a tail of about 90 callees, none over 0.85 M, and the repeated patterns in it are rows of their own below (per-pop sorts, the per-round diff, hint walk). A flat self-instruction view double counts shared callees (mimalloc, memcpy) and is not used.

## 2. Step by step

M: chain-1 (per call) | `d_faces` -O2 on main plus #2010, #2021, #2027, #2032 (2,612 M).

| # | step per popped value | llrm | LLVM (RAGreedy) | what llrm lacks | class |
|--:|---|---|---|---|---|
| 1 | pop, bookkeeping | `BinaryHeap<Queued>` re-keyed from the weight at each push; `waiting` counts and a `popped` set; 0.09 M for 279 pushes | `PriorityQueue` of (stage, size, reg); `ExtraRegInfo` array indexed by vreg: stage and cascade; a stage only grows, so a range never returns to an earlier stage | the monotone stage (a requeue after a rewrite resets `Done` to `Assign` in 1.4% of rounds) | inherent, small |
| 2 | order the registers | `target::order` (1.37 M, 4,573 calls on `d_faces`) then hint votes, `Claims::by`, three sorts, and a `BTreeSet<RegId>` of word bases built per pop: 1.94 M + 0.59 M of sort for 273 pops (9.6 K each) | `AllocationOrder` over the class's cached order, hint first (`getRegAllocationHint`), no sets built | the order and the constant sets of a class built once per class | prove-once: 2.5 M |
| 3 | assign: first free register | `_free` 7.8 M for 2,396 calls (3.2 K); the interference query `LiveUnion::meeting` 8.5 M for 9,882 queries (860 instructions), `_clobbered` 5.0 M for 26,220 (190) | `checkInterference` 550 instructions per query, answered from a union per register with a version (`LiveRegMatrix`) | a version on each union so a query is not asked again (A5: "interference is a union per register with a version") | inherent per query, 1.6x LLVM's |
| 4 | evict | `_evict` 4.41 M for 104 searches (42 K): every register of the order meets the union, then each victim's `movable` search runs (2.51 M, 835 calls, 3 K each: is there another register for it), then `LiveRanges::weight` per victim (1.35 M, 1 K each) | `tryEvict`: one candidate per physreg from the interference cost (`canEvictInterferenceBasedOnCost`: weight below the evictee's, cascade number); a local victim is evicted only if it could move to a free register (`canReassign`, on by default at the pin: `enableRALocalReassignment` answers true; translated); the bill is the maximum victim weight and broken hints, where llrm sums weights and counts a movable victim as 0; weight read from the interval (`calculateSpillWeightsAndHints`, once before allocation) | weights stored in the interval; removing `movable` changes decisions, not only cost | `movable` is our procedure and its removal moves output; `weight` prove-once |
| 5 | recolor | `Coloring::recolor` runs only when no split carved anything, just before a spill (`allocate.rs`, inside `if rewritten.is_none()`): 51 calls against 49 spills at 1 copy, 3.32 M; 10.8 M for 119 on `d_faces`, 10.45 M of it in `crowded` | `tryLastChanceRecoloring`: only for a range at `RS_Done` or unspillable, depth-limited; a spillable range goes straight to spill | already a last-chance stage; matching greedy would mean not recolouring spillable ranges, which changes output | our procedure (a decision difference, not a cost shape) |
| 6 | split | `Split::of`, `local`, `placed`, `per_block`, `pays` per attempt, then `carved_moving` (a new body): 2.56 M + 1.68 M for 50 attempts at 1 copy; `placed` 117.4 M before #1958 and 60.5 M after, carve 23.2 M, on `d_faces` | `SplitAnalysis` once, `SplitEditor` edits the one interval and makes new vregs; `SpillPlacement` on kept bundles (3.3 M for 28,095 activations); 3 `splitAroundRegion` on the diamond | the editor contract (a split edits through a log, A5/A6) | greedy-port |
| 7 | spill | `spilled_apart` 5.15 M for 49 spills at 1 copy (105 K); `materialized` 18.5, `planned` 11.3, `siblings_over` 9.0 on `d_faces`; `_tied` recomputed per spill (1.29 M) | `InlineSpiller::spill`: about 28 K, rematerialization decided on the def | LLVM resolves tied operands in `TwoAddressInstructionPass`, before allocation, and rewrites virtual to physical registers in `VirtRegRewriter` afterwards; llrm re-derives `_tied` per spill | rewrite inherent, `_tied` prove-once |
| 8 | after the rewrite | `Facts::of` 9.04 M for 77 builds (119 K); `changes` 0.49; `HintWalk::update` 0.57 (3.96 on `d_faces`); `_next_value_following` 0.44 (8.0 on `d_faces`); overlap and requeue: the rewrite can take the register of a placed value (1.4% of rounds) | `LiveRangeEdit` hands the new vregs and the touched blocks to the next dequeue; the intervals of other vregs and their assignments are untouched; block frequency and bundles once per function | the edit record / `touched_since` (spec 2.3), per-update costs that depend on what changed; LiveIntervals, SlotIndexes, block frequency and EdgeBundles are separate passes outside RAGreedy's 39.5 M, and `Frequency::of` per round has no LLVM counterpart | 6.0 M prove-once; the rest is the round |

## 3. What is recomputation and what is procedure

Prove-once (a fact that did not change, recomputed): facts update overhead 6.0 M, the per-pop order sets and sorts 2.5 M, weights 1.35 M, `_tied` 1.29 M, `crowded` 1.69 M: 12.8 M of the 37.2 M, 34% of the chain-1 total and about 12% of `regalloc` on `d_faces` (~50 M). Procedure: `movable` 2.51 M, the recolor search 1.63 M, the interference queries, the split path and the spill rewrite (about 10 M in all at 1 copy).

Correction of an earlier reading: greedy does have a recolor stage (`tryRecoloring`); it differs in being a last chance for a range that is about to spill and in being depth-limited, not in existing.

## 4. A model of the monotone-stage procedure at chain-1 scale

The earlier draft of this section assumed a facts update of 25 K a round and a tail of 40 K, and totalled 217 K. Both are the whole result: the facts figure contradicts [the design page](allocator-lir-core.md) (3.5 bounds all the O(touched) work at about 25 M, 1% of the compile, roughly 120 K off a 400 K round), and the tail has no mechanism. With the rows the procedure can reach:

| per round, K instructions | today | procedure only |
|---|--:|--:|
| facts (row 8) | 119 | 119 |
| spill rewrite (row 7) | 68 | 68 |
| evict (row 4, `movable` and `weight` removed) | 58 | 20 |
| recolor (row 5, already last-chance) | 44 | 44 |
| split and carve (row 6) | 56 | 56 |
| order sets and sorts (row 2) | 33 | 3 |
| the rest of the tail (the diff, `Frequency::of`, the hint walk, the overlap check, the requeue) | 112 | 112 |
| total | 490 | 422 |

The rows 2 and 4 can save 68 K (and row 4's changes the output); with the pure items only (2, weights) it is 33 + 11 K. About 380 to 420 K remains. The kill (cannot be modelled under 250 K) is met. What is left is the round: `_allocated` rebuilds `body`, then runs `ranges::changes(before, &body)`, `Frequency::of(&body)`, `Stamp::of`, `required_seen.differing` and `Facts::of`, and the split and the spill rewrite build a new body. LLVM has no such round.

## 5. What this changes in the plan

- The pure part is still worth doing, as the first commit of whatever follows, and nothing else of this page's slice: hoist the constant order sets and sorts of row 2 (2.5 M), store the weight in the interval (1.35 M at 1 copy, 5.2 M on `d_faces`), keep `_tied` per instruction. About 12 M at 1 copy; no output change.
- Matching greedy's eviction (no `movable`, the bill as the maximum weight and broken hints) and dropping recolor for spillable ranges change output and are a decision study (spec 4.3 rows), not a cost step.
- The lever is the round: make the allocator's edit an in-place edit (A5; the editor contract of the design page), so that facts, the diff and the body rebuild leave the round. That is the S4 spike of the architecture page: an in-place spill on an arena body, step 0 attribution done by this page and the design page (837/815 assignments, the fit, the accounting), step 1 the in-place spill with per-spill instructions and a 3x kill.
- Open and cheap before step 1: LLVM's spill and eviction counts on the controlled loops. llrm makes 49 spills on chain-1 (191 instructions); clang emits half the stack operands. If LLVM spills far less there, the decisions are not comparable off `d_faces` and the procedure study is the first work.

## 6. Review questions

1. Answered by the first review: the cost comes from the model, however the procedure is arranged (section 4).
2. Which rows of section 2 are not comparable with LLVM's, because the work is done elsewhere in LLVM (the register coalescer, the spill weights pass, the rewriter)?
3. What would a skilled engineer reading this in three years think was the wrong turn?
