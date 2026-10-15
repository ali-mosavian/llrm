# One allocation: the decisions best-of-six makes, and where one run makes them

For contributors to the register allocator. The measurements come first; the design follows from them and nothing in it is built. The data structures of a round (postings by slot, facts kept across rounds) are the allocation-algorithm note's; this note is the decision structure: what the candidate allocations decide differently, and the in-run step that replaces each.

## What is run today

`assemble::directed` allocates a function on up to two routes (the allocator alone; the spiller's `SsaSpill` first, then the allocator) and keeps the cheaper. `RegAlloc::transform` runs the base allocation and, if it spilled, up to two trials (the first of `Scoped`, `Retainable`, `Addressed`, `Unfolded` that its spills admit, and `Whole`, the body without splitting) and keeps the cheapest. That is up to six allocations of one body. The allocator inside each is already LLVM's `RegAllocGreedy` in shape (priority queue, eviction with cascade numbers, region splitting, recolouring).

## What they buy

Corpus: the 66 vsgcc programs and 65 QCport modules (`tools/identical.py`'s), -O1/-O2/-Os, 4041 allocations; cost is executed work (`allocate::_emitted`), route cost adds two per saved register. Script: `tools/routetable.py` (`LLRM_CANDIDATES=measure`, `LLRM_DEBUG=routes,regalloc,candidates`).

| decision | where | wins | gain when it wins |
|---|---|---|---|
| no splitting (`Whole`) | trial | 132 of 704 at -O1 (19%) | median 1.7%, max 26.5% |
| loop-local copies protected from spilling (`Scoped`) | trial | 53 (26%) | median 4.9%, max 20.5% |
| one spilled invariant base kept (`Retainable`) | trial | 32 (23%) | median 4.9%, max 12.7% |
| address arithmetic apart (`Addressed`) | trial | 11 | median 2.5% |
| folded indexes opened (`Unfolded`) | trial | 1 | 9.5% |
| spiller first (`SsaSpill`, route B) | route | 127 of 422 functions (30%) | mean 1.8%, max 75%; worse in 163 (39%), equal in 132 |

-O2 and -Os are the same within three points. The current gate runs route B on 85% of functions; the cheaper gates measured lose gain: spill>0 7%, spill of at least 1% of weighted instructions 21%, any loop spill 49%, 100 instructions or more 55%. Nothing the base run knows predicts route B.

## What a winning trial changes

Diffing the base run's split and spill sets with the winner's (value numbers are shared with the base body), per win:

| trial | values the base split and the trial did not | held where the base spilled | spilled where the base held | same spills |
|---|---|---|---|---|
| `Whole` (125 wins) | 4.95 | 1.59 | 0.66 | 36% |
| `Scoped` (52) | 1.42 | 1.85 | 2.04 | 10% |
| `Retainable` (30) | 1.53 | 2.20 | 2.10 | 3% |
| `Addressed` (10) | 1.20 | 2.30 | 0.50 | 20% |

- `Whole` is the base's splits undone: five split values a function on average, and net 1.6 fewer values spilled for 0.7 more. The splits were priced one value at a time (`splitkit`'s pay test) against registers their pieces later lose.
- `Scoped` and `Retainable` trade about two values for two others: a different choice of which values keep registers in a loop, by weight, not by more splitting.
- A trial shares 0 to 4 of its first 119 to 189 events with the base: the greedy order is chaotic, so a trial cannot be reused or cut short, and a single run cannot copy a trial's events.

## The decision points, and the step that makes each in one run

Each follows LLVM's `RegAllocGreedy` where it has the step, and says where it does not.

1. **Undo a split that did not pay (replaces `Whole`).** Keep, for every split, what it was priced at (spill weight saved, border copies) and, when the queue is empty, what happened (each piece's register, or a spill, and the copies left after hinting). Rejoin the families whose actual outcome is worse than their unsplit value's, worst first, and re-run only the values the rejoin disturbs (`LiveRangeEdit` in reverse). LLVM has no such step: it prices the split with `calculateRegionSplitCost` at the decision (copies at borders from `SpillPlacement`'s bundles, hints from `RegisterCoalescer`) and accepts it only below the spill cost; the pay test here lacks the border-copy term after hinting. Measure first whether pricing it right at the decision removes the wins without the undo.
2. **Protect a loop's piece while others are evicted (replaces `Scoped`).** At the Split stage, a value live across a loop it is spilled in gets a loop-local piece whose weight is the loop's uses at the loop's frequency, so the eviction step (`tryEvict`) takes a register from lighter holders instead of spilling the piece. This is IRA's loop regions (`ira-build.c` allocates region by region, inner loops first); here it is one split candidate in the run.
3. **Spill weight counts the instruction a spill adds (replaces `Retainable`).** A value whose uses are address bases costs a reload and an instruction per use when spilled, which memory-operand folding does not save. `CalcSpillWeights` already charges folded loads and hints; add the unfoldable address use.
4. **Fold or unfold at the spill (replaces `Addressed` and `Unfolded`).** When a value is spilled, choose per use between reload and use, and opening the folded index (X86 `unfoldMemoryOperand`), by the local cost, inside the spiller's rewrite.
5. **Route B's spill decisions inside the run.** `SsaSpill` spills before allocation by pressure on SSA form. It wins 30% and loses 39%, so it is not better, it is different; the run takes its victims as input to the weights (a value it spills is lowered in the queue), and the route is dropped when the run's output cost over the corpus is at most today's best-of-two. Until then route B stays.

## Quality gate

- Per function, the single run's cost against today's best (`directed` and the trials): the sum over the corpus, and the number of functions worse.
- The bench: 424 rows, none worse; `tools/identical.py` for the objects that change.
- Kill switch `-fallocation-single`, off. Default on only when both hold at -O1, -O2 and -Os.

## Milestones, each with its number

1. The harness: `tools/routetable.py` and a per-function cost report of the single run against best-of-six (this note's tables regenerated). Number: the table above on the current tree.
2. Step 1 (the pay test priced with border copies; then the undo if the wins remain). Number: `Whole` wins, 132 at -O1, and the corpus cost against best-of-six.
3. Steps 2 to 4, one at a time. Number: the wins of the trial each replaces.
4. Step 5 and the route. Number: functions where route B wins and the cost lost without it.
5. Drop the trials and the route behind the switch; compile cost by the table of the allocation-algorithm note.

The compile effect is ×1 allocation instead of up to six on a function with a spill; the postings and facts work cuts each allocation.
