---
architecture-index: one ScalarEvolution per function, held by the manager across the loop passes: design and invalidation map
architecture-order: 521
---
# One ScalarEvolution per function

TL;DR: the pin builds ScalarEvolution once per function and keeps it through every loop pass; llrm builds it at 19 call sites, each from nothing. `indvars` over d_faces -O2 fell from 48.9 M to 25.8 M instructions when its six steps shared one (21 M of the 49.7 M were trip counts worked out again). The rest of the loop passes and LSR follow, through the manager's `Scevs` entry.

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
2. PR 1: indvars and LSR read `Scevs` from the manager and name it; rows pair and d_faces callgrind (`indvars` slot, LSR `IvUsers::of`). Due 2026-10-28.
3. PR 2: rotate, loop-interchange, profit/unroll/peel, loop-load-elim, then loop-idiom, licm, unswitch, as their owners are free (unroll wiring: w-3).
4. Cost still above the pin after sharing (`get_backedge_taken_count` 5x and `isImpliedCond` 7x per loop at LSR) is a separate item: a missing cache or repeated work inside SCEV, measured with callgrind on the same input.

## Not in scope

Predicated ScalarEvolution (D24 of `scev-port.md`) and the loop-access modules; they ask the same SE.
