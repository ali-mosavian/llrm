---
architecture-index: one ScalarEvolution per function, held by the manager across the loop passes: design and invalidation map
architecture-order: 521
---
# One ScalarEvolution per function

TL;DR: the pin builds ScalarEvolution once per function and keeps it through every loop pass; llrm builds it at 19 call sites, each from nothing. Measured 2026-10-22: keeping it across every session on a function (the history log drops what edits touched, `LLRM_VERIFY` checks it) saves 2.7 M Ir of 1,531 M on d_faces -O2 (25 hits in 47 sessions), so sharing is a correctness property of the translation, not a cost item. The SE bill below is per-operation cost (about 1.3x the pin's on the same IR for IndVars and LSR) and passes that run more often than the pin's.

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

## The bill, d_faces -O2

Inclusive callgrind Ir (3.25.1) of the calls a pass makes into `ScalarEvolution`, counting callers outside SE itself so no call is counted twice.

- Ours: w-3's run, `cg.out`, total 1,434,903,106 Ir; `llrm-cc` of d7d4e23a4 (md5 dda9cea7f636, parity plus the D12 revert); input `d_faces_flat.c` (md5 ca759b58a127); `-m32 -mabi=sysv -O2 -march=i486`. Not the ledger's run (c4fade734, 1,317.7 M): never mix the two.
- Pin: `cg-opt.out` (285,166,041 Ir) and `cg-llc.out` (214,484,575 Ir), the pin's `opt` (md5 e8430a02d9bc) and `llc` (906c6268d6e7) on our `01-globalopt` IR of the same input, i386 i486 target.
- Different inputs to the same passes, so read the shape, not the last digit.

| Pass | ours M | pin M | ours in | pin in |
| --- | --- | --- | --- | --- |
| indvars | 32.16 | 12.77 | simplifyindvar 23.71, exitopt 5.14, indvars 2.89, lftr 0.42 | SimplifyIndVar 12.63, IndVarSimplify 0.14 |
| loop-unroll | 9.21 | 3.75 | loopunrollpass | LoopUnrollPass 3.32, LoopUnroll 0.10, LoopPeel 0.33 |
| loop-deletion | 7.59 | 0.05 | loopdeletion (34 fresh SEs) | LoopUtils |
| loop-idiom | 5.68 | 3.72 | loopidiom | LoopIdiomRecognize |
| LSR | 12.35 | 3.65 | ivusers 5.71, lsr_collect 3.81, lsr_pass 1.08, lsr_implement 0.92, lsr_chain 0.40, lsr_formula 0.35, lsr_generate 0.08 | IVUsers 1.99, LoopStrengthReduce 1.66 |
| loop-load-elim and access checks | 6.28 | 2.76 | looploadelim 3.94, MemoryDepChecker 1.23, loopaccess 0.48, runtimecheck 0.57, accessanalysis 0.06 | LoopAccessAnalysis |
| SE's own helpers | 9.46 | 3.71 | scev-other | ScalarEvolution 2.43 (opt), 1.28 (llc) |
| value facts | 0.98 | 0 | valuefacts | none |
| rotate, loop-simplify, SLP, vectorize, constraint elimination | 0 | 3.73 | none in this compile | LoopRotationUtils 1.83, LoopSimplify 0.65, SLP 0.90, ConstraintElimination 0.21, LoopVectorize 0.14 |
| **cold-SE passes** | **83.71** | **34.14** | | |
| decide, algebraic | 10.11 | 0 | decide 8.60, algebraic 1.51 | none |
| **total** | **93.82** | **34.14** | | |

The stated totals are 93.94 M (ours) and 34.23 M (pin): the 0.12 M and 0.09 M left over are callers too small to name.

Measured afterwards (2026-10-22): the bill is not mostly repeated work. A `ScevSession` that took the expressions the last session on the same function left, less what `Function::touched_since` says the edits touched, hit 25 times in 47 sessions and saved 2.7 M (1,531.02 M -> 1,528.31 M, release profile, objects identical). On the same IR, the pin's LSR is 18.39 M and ours 23.9 M (1.3x); IndVars is 12.42 M and 15.95 M (1.28x). The table's pin column is the pin compiling the source, whose loops are its own IR after its own passes, so it overstates the gap; the same-IR ratios do not.

The prize is stated twice:

- Cold-SE passes only: 83.7 - 34.1 = 49.6 M. This is what the persistent SE (steps below) can take.
- `decide`'s and `algebraic`'s SE steps, 10.1 M: a use of SE the pin does not make (its CVP, SimplifyCFG and InstCombine ask no SE), so a translation gap to remove, not part of the persistent SE. Owner and lever: decide@809's scev step is deleted by the loop-pass worklist (w-5, PR 2026-11-09); `algebraic`'s 1.5 M has no owner yet.

Two things are not cold SE either: `get_exact_sdiv` in LSR (1.64 M, 39% of keys repeated, closed, no cache) and the per-visit cost of indvars (1.28x the pin's, 15.95 M against 12.42 M on the same input).

The cold-SE part is IVUsers' `get_scev` (4.4 M over 240 calls: the first build of each SCEV in LSR's own session), `delete_loops` (34 fresh SEs for 17 loops, 7.6 M of backedge-taken work, about 3.8 M if each loop's were worked out once), `unroll_loops` (40 fresh SEs, 7.9 M), and the trip counts again in each pass that builds a session of its own. Of the 41.1 M that `indvars` costs over 17 runs, 32.2 M is SE; the pin's whole IndVarSimplify is 18.0 M (39 loop visits), 12.8 M of it SE.

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
2. Withdrawn as a cost item. Step 1 (one SE per loop in `delete_loops`) measured 5 K Ir: parity returns on `mustprogress` before the dead check builds an SE. Holding the SE in the manager stays a goal for matching the pin's invalidation, with the verify mode first.
3. PR 2: rotate, loop-interchange, profit/unroll/peel, loop-load-elim, then loop-idiom, licm, unswitch, as their owners are free (unroll wiring: w-3).
4. Cost still above the pin after sharing (`get_backedge_taken_count` 5x and `isImpliedCond` 7x per loop at LSR) is a separate item: a missing cache or repeated work inside SCEV, measured with callgrind on the same input.

## Not in scope

Predicated ScalarEvolution (D24 of `scev-port.md`) and the loop-access modules; they ask the same SE.
