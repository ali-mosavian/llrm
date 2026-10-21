---
architecture-index: one ScalarEvolution per function, held by the manager across the loop passes: design and invalidation map
architecture-order: 521
---
# One ScalarEvolution per function

TL;DR: the pin builds ScalarEvolution once per function and keeps it through every loop pass; llrm builds it at 19 call sites, each from nothing. SE work called from passes is 93.9 M Ir of the 1,318 M d_faces -O2 compile; the pin's is 34.2 M (opt 29.3, llc 4.9): about 60 M is the prize. Step 1 (one SE per loop inside `delete_loops` and `unroll_loops`, PR 2026-11-06) is the cheap part; the manager-held SE (PR 2026-11-16) is the rest.

## The pin's rule

`ScalarEvolutionAnalysis` is a function analysis. `ScalarEvolution::invalidate` (`ScalarEvolution.cpp#L15117`) drops it when the pass did not preserve it, or when `AssumptionAnalysis` or `DominatorTreeAnalysis` was invalidated. Every loop pass preserves it (`getLoopPassPreservedAnalyses`, `LoopAnalysisManager.cpp#L141`: dominator tree, loop info, ScalarEvolution), and so does the loop pass adaptor (`LoopPassManager.cpp#L337`). A transform that edits keeps the preserved SE true in two ways: erasing a value or replacing all its uses reaches SE by itself, through the `SCEVUnknown` value handle (`deleted`, `allUsesReplacedWith`); a change in place, or of a loop, is told with `forgetValue`, `forgetLoop`, `forgetTopmostLoop`, `forgetBlockAndLoopDispositions`. MIR has no value handles; the edit log (`Function::changes_since`, `forget_touched`) does the first job.

## Where the pin's SE lives and dies (O2 order)

| Pass | Preserves SE | Source |
| --- | --- | --- |
| loop-simplify | yes | `LoopSimplify.cpp#L844` |
| lcssa | yes | `LCSSA.cpp#L566` |
| licm (both runs) | yes | `LICM.cpp#L314`, `#L351` |
| loop-rotate | yes | `LoopRotation.cpp#L103` |
| simple-loop-unswitch | yes | `SimpleLoopUnswitch.cpp#L3521` |
| loop-idiom | yes | `LoopIdiomRecognize.cpp#L332` |
| indvars | yes | `IndVarSimplify.cpp#L2231` |
| loop-deletion | yes | `LoopDeletion.cpp#L542` |
| loop-unroll, full-unroll | yes | `LoopUnrollPass.cpp#L1723`, `#L1816` |
| loop-reduce | yes | `LoopStrengthReduce.cpp#L7295` |
| loop-interchange | yes | `LoopInterchange.cpp#L2744` |
| constraint-elimination | yes | `ConstraintElimination.cpp#L2708` |
| sroa, early-cse, instcombine, simplifycfg, jump-threading, correlated-propagation, gvn, sccp, bdce, adce, memcpyopt, tailcallelim, reassociate, float2int, instsimplify | no | drops it |

So in the pin's O2 function pipeline one SE serves each run of loop passes between two function passes that drop it (loop-simplify, lcssa, LPM1, then LPM2 and its loop-simplify/lcssa), not one per pass.

## llrm today

The manager has `Scevs` (`scev/analysis.rs`: one `ScalarEvolution` per function, dropped unless a pass names it in its `PreservedAnalyses`). Only `decide` and `algebraic` read it, through `LazyScevView`. Every loop pass builds a `ScevSession` of its own (a `DomTree`, `LoopInfo`, `AssumptionCache` and an empty `ScalarEvolution`): indvars and its steps (one per run since `perf(indvars): one ScalarEvolution for the run`), LSR, rotate (3), loop-interchange, profit/unroll/peel (`scevs`), loop-load-elim, `valuefacts`. No loop pass preserves `Scevs`.

## Design

1. A loop pass asks the manager: `ScevView::of(context, layout, function, analyses)` gives the function's `ScalarEvolution`, loop info, tree and assumptions. No pass builds a session of its own.
2. A pass that edits keeps SE true before it returns, the way LLVM does: `forget_value` for each value it erased, replaced or changed, `forget_loop` for a loop whose trip count or structure changed, `forget_touched` from the edit log (`perf(scev): keep the expressions an edit did not touch`) when the edit is a batch. It then returns `PreservedAnalyses` naming `Scevs` with `Dominators` and `Loops`.
3. A pass that does not keep SE true (or changes the CFG, assumptions, or loop info) does not name it; the manager drops it with the entry's dependencies (`dominators`, `loops`, `assumes`), the pin's rule.
4. A check, not trust: a verify mode (`LLRM_VERIFY`) recomputes each cached expression of a function after a pass that named `Scevs` and compares (LLVM's `ScalarEvolution::verify`, `ScalarEvolution.cpp#L14821`). The manager's replay check cannot do it: `ScevCache::eq` is always true by design. This is the first piece to build, so a missed `forget` fails a test instead of miscompiling.

## The bill, d_faces -O2 (callgrind Ir, 2026-10-22)

Inclusive Ir of the calls a pass makes into `ScalarEvolution` (callers outside SE itself, so no call is counted twice). Ours: w-3's callgrind of the dist of d7d4e23a4 (`~/scratch/w3-cg/cg.out`, d_faces -m32 -O2). The pin: its `opt` on our IR and its `llc` (`~/scratch/ledger2/cg-opt.out`, `cg-llc.out`). Different inputs, so read the shape, not the last digit.

| Pass | ours M | pin M | Pin's pass |
| --- | --- | --- | --- |
| indvars (simplifyindvar 23.7, exitopt 5.1, indvars 2.9, lftr 0.4) | 32.1 | 12.8 | SimplifyIndVar 12.6, IndVarSimplify 0.1 |
| loop-unroll | 9.2 | 3.4 | LoopUnrollPass 3.3, LoopPeel 0.3 |
| loop-deletion | 7.6 | 0.05 | LoopUtils (the pin's BTC is cached) |
| loop-idiom | 5.7 | 3.7 | LoopIdiomRecognize |
| LSR (ivusers 5.7, lsr_* 6.6) | 12.3 | 4.9 | IVUsers 2.0, LoopStrengthReduce 1.7, SE 1.3 |
| loop-load-elim and the access checks | 3.9 | 2.8 | LoopAccessAnalysis |
| decide, algebraic | 10.1 | 0 | none: ours reads SE where the pin's CVP, SimplifyCFG and InstCombine do not |
| SE's own helpers (scev-other) | 9.5 | 2.4 | ScalarEvolution |
| rotate, loop-simplify, SLP, vectorize | 0 | 3.6 | LoopRotationUtils 1.8, LoopSimplify 0.65, SLP 0.9, others 0.3: ours asks SE nowhere in this compile |
| total | 93.9 | 34.2 | |

Two things are not the cold SE: `decide`'s and `algebraic`'s 10.1 M is a use the pin does not make (a numbered DEVIATE of theirs, not this note's), and `get_exact_sdiv` in LSR is 1.64 M with 39% repeated keys (closed, no cache).

The cold-SE part is IVUsers' `get_scev` (4.4 M over 240 calls: the first build of each SCEV in LSR's own session), `delete_loops` (34 fresh SEs for 17 loops, 7.6 M of backedge-taken work, about 3.8 M if each loop's were worked out once), `unroll_loops` (40 fresh SEs, 7.9 M) and the same trip counts again in each of indvars' steps once a session is not shared across runs. Of the 41.1 M that `indvars` costs over 17 runs, 32.1 M is SE; the pin's whole IndVarSimplify is 18.0 M, 12.8 M of it SE.

## Who reads and who invalidates SCEV

Readers that build a session of their own today: indvars (one per run) with simplifyindvar, exitopt, exitvalues, congruentiv, canonexit and lftr; LSR (`ivusers`, `lsr_*`); `loopunrollpass` (40) and `loopunrollandjam`; `loopdeletion` (34); `loopidiom` (5); `looploadelim`; `loopinterchange`; `simpleloopunswitch`; `rotate` (3); `valuefacts`. `decide` and `algebraic` read the manager's `Scevs` through `LazyScevView`. LICM asks no SE.

Passes that edit what SE has seen, and so must forget: indvars (widening, exit values, LFTR), LSR (new recurrences), unroll and peel, loop-deletion, loop-idiom, rotate, unswitch, `fold`, `algebraic`, `dead`, `gvn` and the other function passes: the last group names no `Scevs`, so the manager drops it, as the pin does for the passes in the table above.

## Invalidation map: what each of our slots must forget

| Slot | Edits | Forgets in the pin (count in the file) |
| --- | --- | --- |
| indvars | widens phis, rewrites exit values and conditions, erases dead phis | `IndVarSimplify.cpp`: `forgetValue` 2, `forgetValues` 1, `forgetLoop` 2, `forgetTopmostLoop` 1 |
| loop-reduce | rewrites addresses, adds recurrences, erases dead IVs | none in `LoopStrengthReduce.cpp`; the expander's `replaceCongruentIVs` calls `forgetValue(Phi)` (`ScalarEvolutionExpander.cpp#L1948`) |
| loop-rotate | copies the header, retargets edges | `LoopRotationUtils.cpp`: `forgetValue` 1, `forgetTopmostLoop` 1, `forgetBlockAndLoopDispositions` 2 |
| licm | hoists, sinks, promotes | `LICM.cpp`: `forgetLoopDispositions` 1, `forgetBlockAndLoopDispositions` 1 |
| loop-unroll | clones the body | `LoopUnroll.cpp`: `forgetTopmostLoop` 1, `forgetAllLoops` 1, `forgetValue` 1, `forgetBlockAndLoopDispositions` 1 |
| peel | clones iterations ahead | `LoopPeel.cpp`: `forgetTopmostLoop` 1, `forgetBlockAndLoopDispositions` 1 |
| loop-idiom | replaces a loop by a call | `LoopIdiomRecognize.cpp`: `forgetLoop` 7 |
| loop-deletion | deletes a loop | `LoopUtils.cpp` `deleteDeadLoop`: `forgetLoop` 2, `forgetBlockAndLoopDispositions` 2 |
| simple-loop-unswitch | clones, retargets | `SimpleLoopUnswitch.cpp`: `forgetTopmostLoop` 3, `forgetLoop` 3, `forgetBlockAndLoopDispositions` 4 |

## Order of work

1. The verify mode, with a test that a missed `forget` is caught (fail-first).
2. Step 1, PR 2026-11-06: `delete_loops` and `unroll_loops` ask one SE per loop (the second ask reuses the first's backedge-taken count; about -3.8 M and -2 M). Then PR 2026-11-16: indvars, LSR, unroll, loop-deletion and idiom read `Scevs` from the manager and name it; rows pair and d_faces callgrind.
3. PR 2: rotate, loop-interchange, profit/unroll/peel, loop-load-elim, then loop-idiom, licm, unswitch, as their owners are free (unroll wiring: w-3).
4. Cost still above the pin after sharing (`get_backedge_taken_count` 5x and `isImpliedCond` 7x per loop at LSR) is a separate item: a missing cache or repeated work inside SCEV, measured with callgrind on the same input.

## Not in scope

Predicated ScalarEvolution (D24 of `scev-port.md`) and the loop-access modules; they ask the same SE.
