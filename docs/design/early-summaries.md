# Summaries at fixed points in the early phase

Status: parked. Measured ceiling 0.2-0.6% of the -O1 compile (see Review and measurement). Follows [#1153](https://github.com/ali-mosavian/llrm/pull/1153), which froze the module summaries after the interprocedural step.

## Where the time is

The module analyses `Summaries` and `GlobalsAA`, with the per-function alias analyses under them, are the largest family of the -O1 compile. QCport, 65 modules, main after #1153 (231.5 G instructions):

| row | share |
|---|---|
| call-effects | 5.0% |
| globals-aa | 3.2% |
| summaries (direct, points-to, visit, callbacks, the analysis itself) | 8.6% |
| points-to | 3.4% |
| through-memory | 3.2% |

Both module analyses are `require`d, so the pass manager makes them again before the next pass whenever a pass changed anything. Timing their outermost spans by the pass they precede (230.1 G compile, 12.0% in the spans):

| made before | spans | share of the compile |
|---|---:|---:|
| ports | 101 | 3.0% |
| pipeline | 130 | 2.1% |
| globalopt | 130 | 2.1% |
| dead (the interprocedural step's rerun of a changed body) | 158 | 1.8% |
| decide (the same) | 137 | 1.1% |
| calleepop | 130 | 1.0% |
| freeze (marker after calleepop) | 76 | 0.7% |
| the late passes, after #1153 | 31 | 0.3% |

Two kinds. The first computation of a stretch of passes (globalopt, ports, pipeline, calleepop, freeze: 8.9%) is the whole module, made again because the module pass before it changed the module; two thirds of it was made before a pass that reads nothing and dropped by that pass's edit. [#1182](https://github.com/ali-mosavian/llrm/pull/1182) stops that for module passes and the marker (-1.8% of the -O1 compile, objects identical). The reruns inside the interprocedural step (dead, decide: 2.9%) are made after an inline or a promote that really changed a body, and the summary of its callers with it; that is what this proposal is about.

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

Counted, not estimated: after #1182 the spans that remain are ports 3.0%, pipeline 2.1% (the first of each stretch, needed by the pass that follows) and dead and decide 2.9% (the reruns). The proposal is for the last of these only: three computations per module (after the early pipeline, after the interprocedural step, the last) in place of one per rerun. The reruns are incremental, so the saving is a part of 2.9%, not all of it: **1-2% of the compile at most**. The measurement that settles it is cheap and comes first: the cost of those spans split into what the dirty closure re-derives and what is fixed overhead.

The first computation of a stretch (ports, pipeline: 5.1%) is not helped by this. It is the whole module made again after a program pass or an edit; carrying the analyses' memo across the stretches (tried: `ModuleAnalyses.memos` kept between them) gave nothing measurable, so that cost is in re-deriving the bodies that changed, as in the table above.

## Against a body-level incremental points-to

The alternative is to keep every recomputation and make each cheaper: update the previous points-to solution for the changed instructions of a body. It would help the per-function rows as well as the module ones (a ceiling of 1-2% was estimated from the share of repeated solves, [#1166](https://github.com/ali-mosavian/llrm/issues/1166)), but it is a new incremental solver over a lattice with widening, whose result must equal the from-scratch one (`LLRM_CHECK_*` would run both). Fixed points with merging change what runs and when, and reuse the solver as it is. Do the cheap one first; the incremental solver stays an issue.

## Risks

- Callee-first order changes what each body's early passes see (a callee's summary, rather than the stale one), so objects can change. That is a quality question, answered by clocks, instructions and bytes on bench and QCport, not by identity.
- The merge for an inline must cover what the inlined body does through the parameters, including memory reached by a pointer parameter instantiated to a global or to a local of the caller. Summaries are slices of objects; instantiation is the operation `alias::summaries` already performs at a call, so the merge reuses it rather than a second implementation.
- A summary that is too large (merged, never narrowed) costs precision until the last recomputation. The last fixed point restores it before the late passes, which is where the precision pays.

## Plan

1. Measure: the time of `Summaries` and `GlobalsAA` inside the interprocedural reruns and in the stretches' first computations, separately.
2. If the reruns' share is above 2% of the compile: the merge, the check mode and the test (a pass that adds an access fails it, seen failing), in one PR; the callee-first order in another, because it moves objects.
3. Stop if the first PR buys under 1% of the -O1 compile.

## Review and measurement

Measured: keeping `Summaries` and `GlobalsAA` across every `edited` splice, promote and deadargs in the interprocedural step (an unsound rule, a bound on what any fixed-point scheme can save) gave -0.17% at -O1 and -0.64% at -O2 over bench and QCport, objects identical on 272 programs. The proposal is parked at that ceiling; the "1-2%" above was a guess.

A review of this document against the code found what to correct if it is ever taken up:

- Summaries are already transitive: `_summarized` adds each callee's instantiated summary at its call site, so after an inline the caller's held summary is sound as it is and nothing is merged. The checker, not the summary, is what an inline strains.
- Passes that change a signature or add a function are not "remove only": deadargs and argument promotion renumber parameters, so a held `Parameter(i)` slice or captures entry maps to the wrong actual; cloning adds a function the held summary lacks. They recompute.
- `Summary::covers` (#1153) is built for the late passes: it is lax (a fresh `unknown_*` covers anything; captures and `unknown_write_types` are ignored) and strict (exact slice membership, not containment, so a reshaped slice would raise a false alarm). Early passes need a containment check of its own, with a regression test that fails on a dropped unknown write.
- The dirty set after an edit is larger than "the caller and what is above it": once an exported or address-taken entry is in the closure, every caller of something unknown joins it. `GlobalsAA` is also solved over the whole module; only its per-body facts are kept.
- The saving is not only the module analyses: a recomputed summary that differs gives a new `Outer`, and every body's analyses that read it are dropped. The measurement above includes that.
- gcc: `pass_tail_recursion` runs before the summaries are made ([passes.def:97](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L97)), and the late `pass_local_pure_const` and `pass_modref` ([:368-369](https://github.com/gcc-mirror/gcc/blob/releases/gcc-13.4.0/gcc/passes.def#L368)) refine summaries for callers compiled later in the unit, not "for the next unit".
- Stale summaries also feed the reruns' gvn and dse and the inline-candidate costs, not only the late passes.
