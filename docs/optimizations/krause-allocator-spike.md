# Spike: Krause's tree-decomposition allocator

Question: would an allocator that is optimal on bounded-treewidth CFGs
(Krause, *Optimal Register Allocation in Polynomial Time*, CC 2013; used by
SDCC for z80 and others) beat `allocate.rs`'s greedy-with-eviction, and
where would it plug in?

Sources not read: arxiv and the paper's hosts are blocked here. What follows
about the algorithm is from search summaries and memory; check it against the
paper before building.

## The algorithm, as understood

- Treewidth of the CFG is small for structured code. Take a nice tree
  decomposition of it; a bag holds a few instructions.
- Dynamic programming over the decomposition. A state is an assignment of the
  variables live at the bag's instructions to registers or memory; a state's
  cost is its spills, rematerializations, preference misses and uncoalesced
  moves. Joins merge consistent states.
- Optimal for that cost model. Not restricted to SSA or chordal graphs, and
  it handles register aliasing (AX/AL/AH), which `allocate.rs` also meets.
- Cost: states per bag grow as `(registers + 1)^live`. SDCC bounds the table
  per node and falls back to heuristics when it overflows, so "optimal" holds
  only under that bound.

## Measured: treewidth here is tiny

`LLRM_DEBUG=cfg` (new channel, one line per function at `RegAlloc::transform`)
and `tools/analysis/cfg_treewidth.py`, over `bench/**` with `-O2`: 82 function
bodies, 35 from `llrm-c`, 47 from `llrm-qb`.

| treewidth bound | functions |
|---|---|
| 0 | 41 |
| 1 | 9 |
| 2 | 31 |
| 3 | 1 |

Largest body 30 blocks. The bound is a min-degree heuristic, so it is an
upper bound: the precondition holds with room to spare. Caveat: the suite is
small and optimized code is short; `huge.bas` and real programs may differ.
Doubt this number before trusting it (rule 2): it is the CFG only, and the
DP table is driven by live variables per bag, not by treewidth alone.

## Measured: greedy does spill

`LLRM_DEBUG=regalloc`, same suite (84 allocations, from the `best` line):
52 spill nothing, 32 spill 1 to 37 values. `_bench_matmul` (9 spilled, cost
7153, 540 insns), `_tile_sum` (4) and `_bench_mandel` (6) also log "last
resort" evictions. Any gap is therefore in the 32, and the oracle's first
targets are those: `_lruUse`, `_bench_mandel`, `_bench_matmul`, `_tile_sum`,
`_r_point_leaf`, `_sum_three`. Greedy's spill count is not a gap: an
optimum may spill as many.

## Where it would plug in

`RegAlloc::transform` picks the cheapest of several candidate bodies, each
allocated by `rewritten()` (assign, split, spill, repeat). Everything that
makes llrm's problem irregular is already an input to assignment:
`classes()` (legal registers per value), `pinned`/`fixed`, `_copy_hints`,
and spill/fold prices in `_priority`, `_fold_priced`.

A Krause pass would replace only the *assign* step: same inputs, same
`Assignment` (which already carries `optimal` and `why`), run inside
`rewritten()`. Splitting and spilling stay where they are for now.

## Gaps

1. **Cost model.** The DP needs additive per-instruction costs. llrm's costs
   are interval weights and trial-and-compare of whole outputs (`_emitted`).
   They would have to be restated per instruction, or the DP used only as an
   oracle.
2. **Live-range splitting.** As understood, the DP assigns a variable one
   location, spilling it where needed; llrm splits intervals (`splitkit`) and
   places spills by cost. Whether the DP can express splitting is the open
   question and decides the payoff.
3. **Instruction groups and two-address forms** (`Insn.group`,
   `distinct_roles`, `explicit_selectors`): they must enter as constraints.
4. **Blowup.** 16-bit x86 has few allocatable registers per class, which
   helps; wide live sets in unrolled loops do not. Needs a cap and fallback.

## Measured: the oracle

`backend/exact.rs`: branch and bound for the cheapest set of values to spill
so the rest fit, seeded with greedy's cost. It owns no fact: values, weights,
classes and clobbers are `allocate::Facts`, legality is `_free`, the
candidate registers are `allocate::candidates` (now also greedy's). It prices
what greedy prices, whole values to the stack and no splits.
`LLRM_DEBUG=exact` prints `greedy G exact E proved|unproved` per body.

The search is cut by two bounds, neither of which changes the answer
(`test_exact_bounds_prune_without_changing_the_answer`): values no register
can hold even alone (live across a call) are spilled up front, and at the
most crowded points the values needing a general register must come down to
the registers there are. The first mattered most: the bodies greedy spills
are mostly not crowded, they are clobbered.

After merging main (the suite is now 255 bodies, run by
`LLRM_DEBUG=exact` over `find bench -name '*.c' -o -name '*.bas'`): 85 have
nonzero greedy cost. On 66 the search finished and greedy is optimal on 65.
On the 19 it could not finish it found nothing cheaper on 14 and a slightly
cheaper allocation on 5 (`SUMTHREE%`, `_bench_grep`, `_bench_huge`,
`nbody_fixed` twice), each under 12%.

The proved gap is `LRUUSE` (QB, 15 values): greedy spills #7 and #19 for
0.0445, the optimum spills #8 alone for 0.0324. #7 and #19 do not overlap;
each overlaps #8. Each loses to #8 on its own (0.0193 and 0.0252 against
0.0324) so neither evicts it, but together they cost more than #8. Greedy
compares a victim against one value at a time, never against the sum.

That is a general rule, not a body: an eviction should weigh the victim against
everything that would otherwise be spilled because of it. It is LLVM's
last-chance recoloring (`codegen-improvements.md` section 6), unported here.

## Recommendation

Do not replace the allocator. Build it as an **oracle first**: a test-only
exact solver over the small bodies above, on the same cost model as
`_emitted`, reporting greedy's gap per function. That answers "is there
anything to win" at the cost of one module and no risk to codegen. If the gap
is small, stop; if not, wire it as one more candidate in `transform`'s
trial list, kept only when cheaper, as the others are.

An exact solver by brute-force or ILP would answer the same question at
these sizes. The tree decomposition only matters once bodies get large or the
oracle becomes a pass; that is the point at which Krause's algorithm earns
its complexity.

## Next steps

1. Read the paper; settle gap 2 (can the DP express splitting).
2. Try the rule above in `_evict`/the spill decision: when a value would
   spill, price the values it blocks. Test with `LRUUSE`'s shape (two
   disjoint values each overlapping a heavier third) and the oracle as
   judge: the gap on the suite must shrink and nothing may get dearer.
3. Otherwise greedy holds; put the effort in splitting and spill placement,
   not in a new allocator.
