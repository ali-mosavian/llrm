# The allocator on the LIR core: design for plan step 3.3

TL;DR: `regalloc` is 415.5 M of `d_faces` -O2 (2,612 M), against greedy's 39.5 M. It is 206 rounds at 1.9 M each, not a few slow passes. Representation work (my half: the LIR model and the facts) is bounded at about 25 to 30 M (1%) by the measurements below; the plan's -250 M needs the rounds themselves to cost a sixth as much, which is mostly the split path (greedy-port's half). This page is the joint design; each half is a section, and the steps are in the plan's block format. Status: draft for review, nothing built.

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

## 2. What already exists

- `Facts::of` updates slots (`Indexes::patch`), intervals (`worked_among` over the touched values), sibling and fold prices, masks, widths, classes and hints from the facts before it, for 195 of the 206 builds. Each has a rebuilt-equals-updated assert under `LLRM_CHECK_FACTS`.
- Since [#2038] the class counts and the masks are carried by move, the pinned set is kept, and the postings tell the body current by its blocks.
- Instructions are shared across rewrites where a rename moves nothing ([#2021]); what the stashes hold of old bodies is let go at a phase's end ([#2027]).
- Probed and closed: an arena of instructions (all memcpy charged to the backend was 1.3% of the compile), and an in-place edit of the intervals (see 3.4).

## 3. My half: the LIR model and the facts

What is left of `Facts::of` is not a missing update. Per update (195, bodies of 1,543 instructions, 10.6 touched values, 13 changed instructions) it still costs 340 K instructions, 220 per body instruction, because several stages read the body or a whole table to find what changed.

### 3.1 The edit record

The spiller and the splitter know exactly what they added and removed. Today `ranges::changes(before, body)` finds it again by comparing the two bodies, and every stage below takes its touched set from that. A record made at the edit (instructions gone and added, values touched, blocks changed) replaces the comparison and is what spec 2.3 asks of the editor.

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

To be written by greedy-port: the 180.4 M (placed 117.4: a block table and a placement per attempt; carve 23.2; the interference cache per register and block of spec 4.1; the cost per split of 1.1 M, and the number of splits (120 here; LLVM makes 3 on the diamond input, its count on `d_faces` is not measured)), the early probe and the kill, and what each step moves. The numbers above are the baseline.

## 5. Steps

Plan block format, steps 3.3.a to 3.3.c for my half (owner ssa-spill-m16):

| step | effect | early probe | exit |
|---|---|---|---|
| 3.3.a edit record | up to 5 M | comparison cost | counted: instructions compared per round at most the changed ones |
| 3.3.b per-block versions | about 8 M | blocks examined | counted: blocks examined per ask at most the changed blocks |
| 3.3.c tables O(touched) | about 12 M | entries visited | counted: entries visited per update at most 4 x touched |

The joint exit is the plan's: `regalloc` 150 M or less at -O2, identical output, counted `full_interval_builds = 1` and `visited` at most 40 x edited range + 4 x blocks touched. My half alone cannot reach it (section 1); say so in the review.

## 6. Review questions

1. Is the per-round cost (0.45 M against 1.9 M) the right unit, or should the plan count decisions (rounds) first?
2. Is an edit record the right contract for LIR, given the MIR editor's `Change` log (spec 2.3)?
3. Which of 3.1 to 3.3 should wait for the split path's design, since the splitter produces most edits?

[#2010]: https://github.com/ali-mosavian/llrm/pull/2010
[#2021]: https://github.com/ali-mosavian/llrm/pull/2021
[#2027]: https://github.com/ali-mosavian/llrm/pull/2027
[#2032]: https://github.com/ali-mosavian/llrm/pull/2032
[#2038]: https://github.com/ali-mosavian/llrm/pull/2038
