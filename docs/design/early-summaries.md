# Summaries at fixed points in the early phase

Status: proposal, for review. Follows [#1153](https://github.com/ali-mosavian/llrm/pull/1153), which froze the module summaries after the interprocedural step.

## Where the time is

The module analyses `Summaries` and `GlobalsAA`, with the per-function alias analyses under them, are the largest family of the -O1 compile. QCport, 65 modules, main after #1153 (231.5 G instructions):

| row | share |
|---|---|
| call-effects | 5.0% |
| globals-aa | 3.2% |
| summaries (direct, points-to, visit, callbacks, the analysis itself) | 8.6% |
| points-to | 3.4% |
| through-memory | 3.2% |

Both module analyses are `require`d, so the pass manager makes them again before the next pass whenever a pass changed anything. 642 computations of each over the 65 modules, by the pass they were made before:

| before | computations | what it is |
|---|---:|---|
| globalopt, pipeline, calleepop | 65 + 65 + 65 | the first of each stretch of passes between two program passes |
| ports, freeze | 83 + 38 | the same, and the edit of ports |
| dead, decide | 158 + 137 | the interprocedural step's reruns of a changed body, after an inline or a promote |
| hoist, algebraic, others | ~30 | late; #1153 removed the rest |

86% of the recomputations before #1153 gave the summaries they replaced. Most of the early ones do too, but an inline or a promote really changes a body, and the summary of its callers with it.

## What gcc does

GCC computes a function's modref and pure-const summary once, at the end of its early optimizations, and the passes after read it:

- `pass_local_optimization_passes` ([passes.def:69](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L69)) runs, per function in callgraph order (callees first), the early inliner and `pass_all_early_optimizations`, which ends with `pass_local_pure_const` and `pass_modref` ([:102-103](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L102)). A function's early passes read the summaries its callees already have.
- The IPA passes then run once over the unit: `pass_ipa_cp`, `pass_ipa_sra`, `pass_ipa_inline`, `pass_ipa_pure_const`, `pass_ipa_modref` (the propagation), `pass_ipa_reference` ([:158-166](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L158)).
- The inliner does not make the caller's summary again: it merges the callee's into it (`ipa_merge_modref_summary_after_inlining`).
- A pass in between reads a summary that may be stale but is conservative; none makes one after an edit. `pass_local_pure_const` and `pass_modref` run again after the late passes ([:368-369](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L368)), for the next unit.

## What ours does

The pipeline runs each body to its fixed point in module order, with the summaries the manager last made (stale for the bodies it has already changed). The interprocedural step then inlines and reruns each changed body's pipeline, and after every splice it drops the module analyses (`edited` in `interprocedural.rs`), so the next rerun finds them missing and the manager makes them again: that is the 295 before `dead` and `decide`. Each is incremental (only the dirty bodies and the callers that read them), but the dirty set after an inline is the caller, and everything above it in the call graph.

## Proposal

Three fixed points, and a merge for the edits between.

1. Run the early pipeline over the bodies callee-first (strongly connected components of the call graph, bottom-up), and make a body's summary when its pipeline settles, in the manager that the next body reads. This is the gcc order. Today the order is the module's.
2. In the interprocedural step, an inline of callee `c` into caller `f` replaces `f`'s summary with `f`'s joined with `c`'s instantiated at the call (parameters mapped to actuals), minus nothing: an upper bound. A rerun pass that can only remove memory operations (dead, decide, promote erasing a load or store, deadargs) leaves the summary as it is: stale and sound. Mark nothing unknown unless the merge cannot be made (a callee without a summary, recursion through the SCC).
3. One recomputation at the end of the interprocedural step, which gives the precise summaries; #1153's freeze starts from it.

The passes that may add a memory operation (an inline, the argument-promotion and cloning passes, tail-recursion elimination turning a call into a loop in place) are the ones that merge. Everything else declares `adds_memory_operations() == false`, the contract #1153 introduced; a rerun of the pipeline over a body is a pass for this purpose, and `Fixed` is not entirely one (it contains the inliner's cleanup but not the inliner), so the declaration is on its steps, which the pipeline already enumerates.

Soundness is checked the way #1153's is, with the same `covers` (a fresh read or write that is placed and that the held summary lacks): `LLRM_CHECK_STALE` recomputes after each rerun and each inline in the check mode, and asserts the held or merged summary covers the fresh one. A merge that does not is a bug in the merge. The gate runs it over the bench programs and QCport.

## What it should buy

Today 573 of the 642 computations are early; the proposal makes three per module in the common case (after the early pipeline, after the interprocedural step, and the last), plus the SCC-local ones the first fixed point needs. The family rows above are 24% of the compile; the early share of their cost is not separate in the table, since the early computations are the larger ones (the first of a stretch is the whole module, an inline's rerun is a closure of callers). If the early cost is 70% of the family and it falls by two thirds, the saving is about 70% x 24% x 2/3 = 11%, which would be too good: the per-function rows (call-effects, points-to, through-memory) are driven by the bodies' own edits, which this does not change. Counting only the module rows (globals-aa, summaries: 11.8%), 70% early, two thirds saved: **about 5%**. The estimate has a wide error; the measurement that settles it is cheap and comes first: the cost of `Summaries` and `GlobalsAA` in the interprocedural reruns alone, by timing those spans.

## Against a body-level incremental points-to

The alternative is to keep every recomputation and make each cheaper: update the previous points-to solution for the changed instructions of a body. It would help the per-function rows as well as the module ones (a ceiling of 1-2% was estimated from the share of repeated solves, [#1166](https://github.com/ali-mosavian/llrm/issues/1166)), but it is a new incremental solver over a lattice with widening, whose result must equal the from-scratch one (`LLRM_CHECK_*` would run both). Fixed points with merging change what runs and when, and reuse the solver as it is. Do the cheap one first; the incremental solver stays an issue.

## Risks

- Callee-first order changes what each body's early passes see (a callee's summary, rather than the stale one), so objects can change. That is a quality question, answered by clocks, instructions and bytes on bench and QCport, not by identity.
- The merge for an inline must cover what the inlined body does through the parameters, including memory reached by a pointer parameter instantiated to a global or to a local of the caller. Summaries are slices of objects; instantiation is the operation `alias::summaries` already performs at a call, so the merge reuses it rather than a second implementation.
- A summary that is too large (merged, never narrowed) costs precision until the last recomputation. The last fixed point restores it before the late passes, which is where the precision pays.

## Plan

1. Measure: the time of `Summaries` and `GlobalsAA` inside the interprocedural reruns and in the stretches' first computations, separately.
2. If the early share is above 50% of the module rows: the merge, the check mode and the test (a pass that adds an access fails it, seen failing), in one PR; the callee-first order in another, because it moves objects.
3. Stop if the first PR buys under 1% of the -O1 compile.
