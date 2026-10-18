---
architecture-index: the allocator on the LIR core
architecture-order: 180
---
# The allocator on the LIR core: design for plan step 3.3

TL;DR: `regalloc` is 415.5 M of `d_faces` -O2 (2,612 M), against greedy's 39.5 M. The decisions are comparable (837 assignments against LLVM's 815, 67 evictions against 78); the rounds cost 2.0 M each against LLVM's 150 K, and a fit over bodies of 191 to 4,664 instructions says a round costs 431 K plus 309 instructions per body instruction. The sites I and greedy-port have measured add to about 55 M of new work beside the 57 M of #1958; the fit says 57% of a round on the large function scales with the body and a fixed 431 K does not (section 1.1). This page is the joint design; each half is a section, and the steps are in the plan's block format. Status: draft after review, nothing built.

> **Reference note.** The LLVM allocator figures on this page (greedy 39.5 M, 815 assignments, 78 evictions, 122, 13/22/33 K per decision) were taken from the distro `llc` 20.1.8, are not reproduced on the pinned or the distro `llc`, and are being re-taken on the pinned 24.0.0git `d1106deb71cc` by regalloc-core-poc. Do not quote them until its old-versus-new lands.

## 1. Where the 415.5 M goes

`d_faces` -O2, 11 functions, callgrind on main plus [#2010], [#2021], [#2027], [#2032] (instructions, inclusive). A round is one spill, split or rewrite followed by a rebuild of the facts: 206 facts builds, 11 of them from scratch.

| part | M | per round (206) | owner |
|---|--:|--:|---|
| split path (`placed` 117.4, local 10.1, pays/per-block/spread 24.0, `carved_moving` 23.2, next value 5.7) | 180.4 | 0.88 M | greedy-port |
| facts (`Facts::of` 70.7 after [#2038]; Frequency 5.0; hints 4.0; body clone and drops 3.5) | 83.2 | 0.40 M | ssa-spill-m16 |
| spiller (`materialized` 18.5, `planned` 11.3, `siblings_over` 9.0) | 38.8 | 0.19 M | spiller: shared |
| colouring (`recolor` 10.8, `_free` 7.7, a free-register collect 7.7, weights 5.2, live union 3.4, sorts 1.8) | 36.6 | 0.18 M | greedy-port |
| other (constrain, edge bundles, B-trees) | 12 | | |
| `_allocated` self | 10.3 | | |

LLVM's greedy is 39.5 M for the same bodies. The cost per decision, not the number of passes over the function, is the gap: the plan's exit (`regalloc` 150 M or less, of which about 50 M is the base allocation) is about 0.5 M per round, a quarter of today's, with the same number of rounds.

### 1.1 What scales with the body: a fit, and a hypothesis refuted

Experiment (instructions, callgrind, deterministic): one function of K sequential copies of the same loop (24 statements over a shared array and two shared scalars, each copy its own accumulator), K = 1 to 32, so the decisions per copy stay about the same (rounds per copy 76, 95, 99, 87, 76, 63) and only the body grows.

| copies | instructions | blocks | rounds | `_allocated` M | per round, K instr | `Facts::of` per round, K |
|--:|--:|--:|--:|--:|--:|--:|
| 1 | 191 | 6 | 76 | 37.2 | 490 | 119 |
| 2 | 366 | 11 | 190 | 92.7 | 488 | 118 |
| 4 | 685 | 21 | 395 | 236.1 | 598 | 138 |
| 8 | 1,305 | 41 | 696 | 560.6 | 805 | 191 |
| 16 | 2,452 | 81 | 1,221 | 1,441.7 | 1,181 | 260 |
| 32 | 4,664 | 161 | 2,018 | 3,774.8 | 1,871 | 440 |

- The kill (cost per round does not scale with the body at fixed decisions) is not met. The fit is linear: a round costs about 431 K instructions plus 309 per instruction of the body (R^2 above 0.99; at 8 and 16 copies the fit gives 834 K and 1,188 K against 805 K and 1,181 K).
- At 1x to 2x the fixed part is 88% of a round, so the first doubling shows nothing; the scaling shows from about 700 instructions. At the 1,819 instructions of `d_faces`' large function the fit gives 993 K per round, 57% of it size-proportional; the real function costs about 2.0 M per round, twice the synthetic one at the same size: the difference is the split path (carves, `placed`), which the loop does not exercise.
- So the fixed part is the larger surprise: 431 K instructions of every round, on a 191-instruction body, against LLVM's 150 K per decision. Editing O(touched) removes the size-proportional part (at most 57% of a round on the large function, by this fit); the fixed part needs its own attack (the same components at 1x: `Facts::of` 24%, `spilled_apart` 14%, the evict search 12%, the split path 7%, the carve 4.5%, and they keep their shares at 32x).
- An earlier wall-time fit over the corpus (88 microseconds plus 0.20 per instruction, R^2 0.51, 300 allocations) is superseded: it put the size-proportional share at 72 to 80%, which this experiment does not support for the loop.
- Knock-on unassignment is not the cause (a rewrite taking the register of an already-placed value): 104 of 7,694 rounds on the corpus (1.4%), none on `d_faces`. Assignments: 837 on `d_faces` (LLVM 815), evictions 67 (78), queue pops 1,267, requeues after a rewrite 371. The decisions are comparable; the cost of a decision is not.
- Only two of the 11 `d_faces` allocations have rounds: 162 (1,819 instructions, 274 blocks) and 33 (164 instructions); the large one is 90% of the phase.

## 2. What already exists

- `Facts::of` updates slots (`Indexes::patch`), intervals (`worked_among` over the touched values), sibling and fold prices, masks, widths, classes and hints from the facts before it, for 195 of the 206 builds. Each has a rebuilt-equals-updated assert under `LLRM_CHECK_FACTS`.
- Since [#2038] the class counts and the masks are carried by move, the pinned set is kept, and the postings tell the body current by its blocks.
- Instructions are shared across rewrites where a rename moves nothing ([#2021]); what the stashes hold of old bodies is let go at a phase's end ([#2027]).
- Probed and closed: an arena of instructions (all memcpy charged to the backend was 1.3% of the compile), and an in-place edit of the intervals (see 3.4).

## 3. My half: the LIR model and the facts

### 3.0 The backend invariants, and what this half moves toward

LLVM's allocator is cheap per decision because of five invariants of the machine function. Where llrm stands, measured or read in the code:

| LLVM invariant | llrm today | this half |
|---|---|---|
| `SlotIndexes` edited in place: an inserted instruction takes a slot between its neighbours, nothing renumbers | `Indexes::patch`: kept instructions keep their slots, found by comparing the two bodies (4.6 M); `GAP` spacing exists | moves toward: 3.1 and 3.2 feed it an edit record and per-block versions instead of a comparison |
| `LiveIntervals` edited in place (`LiveRangeEdit`, `SplitEditor`: extend, shrink, new vregs only) | `LiveRanges::edit` swaps in intervals recomputed from occurrences for the touched values; 52% come out unchanged | works around: the recompute stays (avoidable at most 11 M, and 56% of changed segments are propagated); not in this design (3.4) |
| instructions with stable handles, use lists maintained by the editor | `Insn::id()` is stable and never reused; occurrences are the thread-local `postings`, brought up to date by comparing blocks (18 M in the facts stages) | moves toward: 3.2 (the editor, not a comparison, says what changed) |
| passes work on dirty regions | the allocator's tables are updated for touched values; 195 of 206 builds are updates; the LIR clean-up visits every block (96.5% idle, plan 3.4) | moves toward: 3.3, tables O(touched) |
| preserved sets per machine pass | none: what a body's facts hold is let go at a phase's end ([#2027]) | not addressed; the facts store is per function, not per pass |

### 3.0.1 The vertical slice that proves it

Before 3.1 to 3.3 are built in full, one slice on one function family, `d_faces`' largest function (`D_DRAW_FACES`, 1,980 instructions): thread the spiller's edit record into `Facts::of` for three stages (widths, hints, siblings) and the postings, with per-block versions on the blocks the spiller rewrote. Measure the instructions per facts update on that function (today about 340 K averaged over the file, to be measured on this function alone).

- Pass: the three stages and the postings read cost at most 4 x touched values + 1 K, and the update total falls by 40% or more.
- Kill: under 20% fall, or the record cannot be made at the edit without a body comparison in the splitter. Then the representation work stops at what [#2038] did, and the allocator's cost is recorded as decisions (plan 3.3, "no effect").

What is left of `Facts::of` is not a missing update. Per update (195, bodies of 1,543 instructions, 10.6 touched values, 13 changed instructions) it still costs 340 K instructions, 220 per body instruction, because several stages read the body or a whole table to find what changed.

### 3.1 The edit record

The spiller and the splitter know exactly what they added and removed. Today `ranges::changes(before, body)` finds it again by comparing the two bodies, and every stage below takes its touched set from that. The contract is spec 2.3's, not a new one: the LIR editor logs the same `Change` records and answers `touched_since(mark)` (the touched blocks and values), as the MIR editor does; a bespoke edit record beside it would be a second contract. It also gives rollback for split pricing. The splitter builds a new `LirBody` per carve today (4.2), so the log is made where the carve edits the touched blocks.

- Early probe: count the instructions `changes` compares per round against the instructions changed (plan: `full_interval_builds`, `visited`). Kill if the comparison is under 5 M.
- Mechanism in LLVM: `LiveRangeEdit` (`InlineSpiller.cpp`) and `SplitEditor` hand the new virtual registers and the touched blocks to the next dequeue.

### 3.2 Postings and numbering per block

`postings::following` is asked about five times per update and costs 18 M in the facts stages (5.8 M of it the postings' own update), and `Indexes::patch` 4.6 M. Both are told what changed by comparing blocks; with per-block versions on `Block` (a counter bumped by the editor, spec 2.1) a stage asks "which blocks changed since version v" in time proportional to the changed blocks.

- Bound (an estimate, to be set by the probe): the reads and the numbering that are not the postings' own update, about 8 M of the 22.6 M.
- Early probe: blocks examined per ask against blocks changed. Kill if the examined share is under 10 M.

### 3.3 Tables that are O(touched)

Widths 7.7, siblings 6.6, hints 3.0, folds 2.2, class counts 4.3, masks 2.5 and slots 5.3 M are each dominated by reads of the postings or a table the size of the function. Held per value in the facts and edited for the touched values only, they cost the touched part.

- Bound (an estimate, to be set by the probe): about 12 M of their 31.6 M.
- Early probe: table entries visited per update against touched values. Kill if the avoidable share is under 8 M.

### 3.4 Not in this design

- An in-place edit of the intervals: 52% of touched values end with an identical interval and 76% of segments are unchanged, but 56% of the changed segments are in blocks that hold no changed instruction (liveness propagated). The recompute is 37.4 M; the avoidable part is about 11 M at most and 6 to 7 M in practice, below the plan's own kill. Buffer reuse (3 to 4 M) goes in the first PR that touches `intervals.rs`.
- The arena and MIR-shaped operands (spec 2.1): no measured saving left in copies. Revisit if 3.1 to 3.3 miss.

### 3.5 My half in numbers

| item | bound | probe kill |
|---|--:|---|
| 3.1 edit record | up to 5 M | comparison under 5 M |
| 3.2 per-block versions | about 8 M | examined share under 10 M |
| 3.3 tables O(touched) | about 12 M | avoidable under 8 M |
| total | about 25 M (1%) | |

## 4. Their half: the split path

### 4.1 What LLVM does on the same function, measured

`llc -O2 -mtriple=i486-pc-linux-gnu` on clang's IR of `d_faces.c` (Watcom keywords defined away; callgrind, `libLLVM.so.20.1` symbols): the whole backend is 327.0 M; `RegAllocBase::allocatePhysRegs` is 30.3 M inclusive (9.3%), against our `regalloc` 415.5 M. Its operation counts:

| operation | calls | M |
|---|--:|--:|
| `LiveRegMatrix::assign` (a vreg gets a register) | 815 | 1.73 |
| `LiveRegMatrix::unassign` (an eviction) | 78 | 0.12 |
| `LiveRangeEdit::createFrom` (a new vreg: a split piece or a spill) | 122 | 0.05 |
| `LiveRangeEdit::eliminateDeadDefs` / `rematerializeAt` | 141 / 113 | 0.60 / 0.18 |
| `LiveRegMatrix::checkInterference` | 6,065 | 3.35 (550 each) |
| `SpillPlacement::activate` / `addConstraints` / `iterate` | 28,095 / 2,608 / 1,741 | 1.25 / 0.91 / 1.11 (45 / 350 / 640 each) |

So the number of decisions is not what separates the two: LLVM makes about 120 new vregs and 78 evictions, we make 120 splits (108 attempts, 74 carved) and 107 spills in 206 rounds. The cost per decision does: LLVM 30.3 M for about 200 decisions is about 150 K a decision, ours 415.5 M over 206 rounds is 2.0 M a round (section 1). The answer to review question 1 is the per-decision cost.

### 4.2 The backend invariants, and what this half moves toward

| LLVM invariant | llrm today (d_faces -O2) | this half |
|---|---|---|
| `SplitAnalysis` reads the value's uses and live blocks once; `SplitEditor` edits that one interval and makes new vregs | `Split::of` once per attempt (shared by `local`, `placed`, `pays`; 10.8 M); the carve rewrites the touched blocks but derives liveness of the value over the whole body (`live_of`), `Occurrences::rows`, `Blocks::of`, `with_blocks`: O(body) per carve (29.1 M for 74 carves, 0.39 M each) | moves toward: 4.4 (the carve takes the allocator's liveness and postings); works around: the carve still builds a new `LirBody` where LLVM edits one interval and inserts instructions |
| `InterferenceCache`: per (physreg, block), refreshed only when the register's union changed (a revision number) | `Occupied` is made per split value; `cursor`/`gather` per register per split (660 gathers, 3.6 M; `Cursor::at` 10.4 M with `interference` 5.3 M of it); nothing survives to the next split although a round changes one value's holder | moves toward: 4.3 |
| `SpillPlacement` on bundles, iterated on the candidate only | `Placement` per attempt (bundles are a cached fact since [#1954]); `Frame` rows per attempt (53 K to build, 105 rows a `grow`, 506 instr a row visit) | already the shape: 4.5 says what is left and why it is not a lever |
| a candidate's cost from bundle states, one `SplitEditor` for the best | `Frame::price` per candidate, one `Region` for the best ([#1958]) | already the shape |
| live intervals and slot indexes edited in place, not rebuilt | section 3 (ssa-spill-m16) | not this half |

### 4.3 The interference cache (4.3.a)

Per (register, union revision): the sorted holders of a register and the points that destroy it, gathered once for every split value asked until the register's holders change. Today 3.6 M (`gather`, 660 calls) + 3.3 M (`cursor`, 750) + the queries 5.3 M; the cache removes the gathers after the first for a register and most of the cursors: about 7 to 9 M (0.3%).

- Early probe: per function, `gather` calls against distinct (register, holder-set) pairs. Kill if the repeat ratio is under 2: the cache would never hit.
- This is below the 1% line by itself: it goes in only with 4.4, or not at all.

### 4.4 The carve from the allocator's postings (4.4.a)

`carved_moving` takes `facts.live` (the intervals) and `postings` (where the value is named) that the allocator already holds, instead of `live_of(body, value)`, `Occurrences::rows` and a `Blocks` per carve; `_next_value_following` becomes a counter the allocator keeps (5.6 M). The block rewrite is unchanged (already local).

- Bound: 29.1 M to about 12 M, plus 5.6 M: about 22 M (0.8%). With 4.3: about 30 M (1.1%).
- Early probe: instructions of `carved_moving` outside the touched blocks' rewrite, per carve, against the body's blocks. Kill if the O(body) share is under 15 M.
- Exact by construction; oracles `LLRM_CHECK_SPLIT`, `LLRM_CHECK_POSTINGS`, `LLRM_CHECK_LIVE`, and the identical corpus.

### 4.5 What was measured and is not a lever

- Position tables for the bundle rows in `Frame` (B'): -0.03% on `d_faces` ([spec 8]): the hash lookups are not the cost.
- `placed` (60.5 M after [#1958], 117.4 M before) is eight pieces of 3 to 10 M, each the work the rows ask for (`price` 10.1, interference 10.4, `held` 6.7, `Frame::new` 5.7, `grow` ~9, `iterate` 3.9). LLVM does the same work per candidate (`calcSplitConstraints`, then `SpillPlacement`), and its `SpillPlacement` is 3.3 M for 28 K activations because the bundles are kept; ours is not the shape that is slow. [#1958] is the lever that was there (-57 M, merge it).
- `spilled_apart` per use, `materialized` 18.7 M: the spiller, shared with ssa-spill-m16 (the plan's E, 0.35%).

### 4.6 The bound

New work measured so far: section 3 about 25 M (1%) and this section about 30 M (1.1%), beside [#1958]'s 57 M already in the queue: about 112 M of the 265 M the plan asks for. The rest is the per-round structure: 206 rounds, each a body rewrite, a facts update that still reads tables the size of the function (340 K instructions an update, 220 per body instruction), a `Frequency`, a stamp and a `changes` comparison of the whole body, and a queue rebuild. Section 1.1's fit puts 57% of the large function's round in what scales with the body and 431 K per round in what does not: O(touched) editing removes at most the first, and the fixed part is a second problem; the vertical slices (3.0.1, 4.7) must be read as tests of the fit and not of one site: a slice that passes on its own site and leaves the round cost unchanged proves only that site. The in-place interval edit (3.4) is not the missing architecture: `LiveRangeEdit` and `SplitEditor` recompute the new vregs' ranges from their uses too, as `LiveRanges::edit` does.

### 4.7 The vertical slice before the full build

One function, the largest (`D_DRAW_FACES`), one family of decisions, end to end: the splits that carve (74 of 108 attempts). Build 4.4 and 4.3 for that function's allocation only (behind the function's size, as the walker's shortcut is), and measure the cost per carved split on it. Today 1.1 M a split (placed 0.56 + carve 0.39 + pays/spread/local 0.2); pass: at most 0.5 M. Kill: above 0.8 M, which says the O(body) share is not where the cost is and the split path is closed as a constant factor (recorded in [spec 8]).

## 5. Steps

Plan block format, steps 3.3.a to 3.3.c for my half (owner ssa-spill-m16):

| step | effect | early probe | exit |
|---|---|---|---|
| 3.3.a edit record | up to 5 M | comparison cost | counted: instructions compared per round at most the changed ones |
| 3.3.b per-block versions | about 8 M | blocks examined | counted: blocks examined per ask at most the changed blocks |
| 3.3.c tables O(touched) | about 12 M | entries visited | counted: entries visited per update at most 4 x touched |

Steps 3.3.d and 3.3.e for the split path (owner greedy-port):

| step | effect | early probe | exit |
|---|---|---|---|
| 3.3.d carve from the allocator's postings, next-value counter | about 22 M | O(body) share of `carved_moving` | counted: body blocks visited per carve at most the touched blocks + 4 |
| 3.3.e interference cache (only with 3.3.d) | about 8 M | gather repeat ratio | counted: gathers per (register, holder-set) at most 1 |

The joint exit is the plan's: `regalloc` 150 M or less at -O2, identical output, counted `full_interval_builds = 1` and `visited` at most 40 x edited range + 4 x blocks touched. My half alone cannot reach it (section 1); say so in the review.

## 6. Review

Reviewed by a second model (Opus), no code read beyond the page's own references. Its points, and what the page now does:

- The unit: per round normalised by body size (section 1.1 does it).
- The contract: spec 2.3's `Change` log and `touched_since`, not a bespoke record (3.1 now says so).
- Order: 3.1 and 3.2 wait for the carve to edit through the log (4.4); 3.3 can go on the touched set the allocator has.
- The central claim: the sum of sites guaranteed a small answer; the experiment of 1.1 replaces it: a size-proportional part of at most 57% of a round, and a fixed part of 431 K.
- Omitted: whole-body work per round besides the facts (`Frequency::of`, `Stamp::of`, `required_seen.differing`, `_next_value_following`, `ranges::changes`) is in the round and now in 4.6; knock-on unassignment measured and refuted (1.1).
- Not answered: the number of candidates per split and `calcSplitConstraints` calls on both sides (section 4.5's claim that `placed` is not the slow shape rests on a count nobody made).

The experiment the review proposed (the same function at 1x, 2x and 4x and beyond, same decisions per copy) is in 1.1.

[#2010]: https://github.com/ali-mosavian/llrm/pull/2010
[#2021]: https://github.com/ali-mosavian/llrm/pull/2021
[#2027]: https://github.com/ali-mosavian/llrm/pull/2027
[#2032]: https://github.com/ali-mosavian/llrm/pull/2032
[#2038]: https://github.com/ali-mosavian/llrm/pull/2038
[#1958]: https://github.com/ali-mosavian/llrm/pull/1958
[#1954]: https://github.com/ali-mosavian/llrm/pull/1954
[spec 8]: compile-time-spec.md

## 7. The greedy port (current)

The design above is superseded: the allocator is now the translation of LLVM's greedy at `d1106deb71cc`, behind `LLRM_GREEDY` on `alim/poc/liveness-types`, and becomes the default only when rows hold at m16 and m32.

Acceptance item from [#2260]: the `lir loopslots` row (x5.82 per doubling) is replaced by this port, with SplitKit's region splitting as its counterpart. With greedy the default, the loopslots step is gone and no step on the cases or nest axes exceeds its limit at 2N (`tools/test_budgets.py`).

Acceptance at 4N ([#2265], debts in [#2260]): each step within its limit under greedy, own Minstr 2N to 4N, `LLRM_GREEDY=1` (e1db028db).

| axis | step | old | greedy | counterpart |
|---|---|--:|--:|---|
| nest | intervals walk | x1.73 | x3.16 | not only allocator: coalesce and twoaddr (lir-cleanup-poc), `Facts::of` and `slots_colored` (greedy: LiveIntervals, LiveStacks) |
| nest | intervals liveness | x3.31 | x4.43 | as above |
| nest | spill cleanup | x2.03 | x2.90 | InlineSpiller without `spiller.rs` (step 2) |
| nest | spill rewrite | x1.74 | x1.55 | within |
| joins | spill slots shared | x2.36 | x2.22 | within; StackSlotColoring/LiveStacks |
| unrolled | regalloc frequency | x2.11 | gone | MBFI |
| unrolled | siblings depths | x2.59 | gone | none needed |
| unrolled | facts masks | x1.81 | x2.00 | within; regmask slots |

Owners of the callers of `intervals walk` and `intervals liveness` under greedy at 4N: `coalesce::joined` and `twoaddr` (lir-cleanup-poc: RegisterCoalescer, TwoAddressInstructionPass); `Facts::of` in the greedy driver and `slots_colored` (regalloc-core-poc: LiveIntervals, LiveStacks); spiller short updates (regalloc-core-poc: InlineSpiller without `spiller.rs`).

x87 ([#2304], slot 60 split): `floatassign` and `float-facts` are replaced by greedy allocating x87 values as RFP32/RFP64/RFP80 virtual registers onto FP0-FP6, as LLVM does before `X86FloatingPoint`; acceptance: under greedy those steps are gone and the float rows hold. The stackifier is slot 60's (backend-slopes); greedy keeps its contract: calls end with the x87 stack empty (results as unnamed FloatLoad, returns as unnamed FloatStore), and no x87 value is live across a call (every FP register in the call's regmask).

[#2260]: https://github.com/ali-mosavian/llrm/issues/2260
[#2304]: https://github.com/ali-mosavian/llrm/pull/2304
[#2265]: https://github.com/ali-mosavian/llrm/issues/2265
