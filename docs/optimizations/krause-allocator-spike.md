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

Same suite: 28 of the 82 bodies have nonzero greedy cost. On 25 the search
finished, and **greedy is optimal on 24 of them**. The exception is
`SUMTHREE` (QB, 42 values): greedy 0.250, optimum 0.218, 13% dearer, proved.
Three bodies (`BLIT&`, and two `__main` of 37 and 208 values) still exhaust
2M nodes with nothing cheaper found.

Under whole-value spilling, then, greedy's eviction is near-optimal on this
suite. The spill-only model cannot see what splitting would win, and
`SUMTHREE` is the one body worth reading to learn what eviction missed.

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
2. Read `SUMTHREE`: dump greedy's and the optimum's spill sets and say what
   eviction missed; fix that if it is a general rule (rule 6).
3. Greedy holds on the rest, so put the effort in splitting and spill placement
   (`codegen-improvements.md` section 6), not in a new allocator.
