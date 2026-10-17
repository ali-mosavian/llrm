# Fact placement: the whole-function tower is solved at named positions

TL;DR: the frequent passes (`fold`, `decide`, `promote`, `sroa`, `hoist`) ask the whole-function fact tower and pay a rebuild per body version; LLVM's `InstCombine` asks six cheap per-query analyses and preserves the CFG ones. This page is the design for S1 (architecture-parity A1): the tower is solved at a few named positions, a frequent pass reads the held result while it is current and "unknown" otherwise. The first probe on `d_faces` -O2 saves up to 9.0% for 23% of objects changed; the rows tool has not judged it, and the clean part is bounded at `fold` and `algebraic`: -2.9%, 3 of 803 objects, rows flat except `x_transpose` +1.0% clocks (bounded at `fold` alone: `tile` +36.7%, killed).

## LLVM structure, ours, and which we mirror

**LLVM.** A pass names what it needs and what it keeps. `InstCombinePass::run` asks the assumption cache, the dominator tree, the target library and cost info, and `AAManager`, whose queries are answered one pointer pair at a time; it skips itself when nothing changed since its last run (`LastRunTrackingAnalysis`) and, when it changed the body, preserves `CFGAnalyses` ([`InstructionCombining.cpp#L6258-L6292`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstructionCombining.cpp#L6258-L6292)):

```cpp
PreservedAnalyses InstCombinePass::run(Function &F,
                                       FunctionAnalysisManager &AM) {
  auto &LRT = AM.getResult<LastRunTrackingAnalysis>(F);
  // No changes since last InstCombine pass, exit early.
  if (LRT.shouldSkip(&ID))
    return PreservedAnalyses::all();

  auto &AC = AM.getResult<AssumptionAnalysis>(F);
  auto &DT = AM.getResult<DominatorTreeAnalysis>(F);
  auto &TLI = AM.getResult<TargetLibraryAnalysis>(F);
  auto &ORE = AM.getResult<OptimizationRemarkEmitterAnalysis>(F);
  auto &TTI = AM.getResult<TargetIRAnalysis>(F);

  auto *AA = &AM.getResult<AAManager>(F);
  auto &MAMProxy = AM.getResult<ModuleAnalysisManagerFunctionProxy>(F);
  ProfileSummaryInfo *PSI =
      MAMProxy.getCachedResult<ProfileSummaryAnalysis>(*F.getParent());
  auto *BFI = (PSI && PSI->hasProfileSummary()) ?
      &AM.getResult<BlockFrequencyAnalysis>(F) : nullptr;
  auto *BPI = AM.getCachedResult<BranchProbabilityAnalysis>(F);

  if (!combineInstructionsOverFunction(F, Worklist, AA, AC, TLI, TTI, DT, ORE,
                                       BFI, BPI, PSI, Options)) {
    // No changes, all analyses are preserved.
    LRT.update(&ID, /*Changed=*/false);
    return PreservedAnalyses::all();
  }

  // Mark all the analyses that instcombine updates as preserved.
  PreservedAnalyses PA;
  LRT.update(&ID, /*Changed=*/true);
  PA.preserve<LastRunTrackingAnalysis>();
  PA.preserveSet<CFGAnalyses>();
  return PA;
}
```

The whole-function solves (SCCP, GVN's memory dependence, CVP, IndVars, SCEV) run at their own positions in the pipeline; a frequent pass does not trigger them.

**Ours.** A frequent pass asks the tower itself. `fold` builds `Held` (`Annotated`, `Pointers`, `Counted`, `Followers`, `Registers`) and `ThroughMemory` for its integers and `FloatFacts` for its floats, and drops everything after it replaces a value ([`fold.rs#L184-L193`](https://github.com/ali-mosavian/llrm/blob/269fdf3d11aedeeea6e933b9181c922dc1b17fe8/crates/opt/llrm-transforms/src/fold.rs#L184-L193)):

```rust
    // Before the facts are asked, so that they see the constants.
    let edges = edges::conditions_decided(context, function);
    if edges {
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let numbers = _numbers(context, layout, function, analyses, calls);
    if numbers {
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    floatfold::_folded(context, layout, function, analyses, calls) | numbers | edges
```

`decide` asks `Held`, `Bounded` and `ThroughMemory` before it looks at a branch ([`decide.rs#L160-L169`](https://github.com/ali-mosavian/llrm/blob/269fdf3d11aedeeea6e933b9181c922dc1b17fe8/crates/opt/llrm-transforms/src/decide.rs#L160-L169)):

```rust
    let threaded = _threaded(context, function) | _phi_threaded(context, function)?;
    if threaded {
        analyses.invalidate(function, &PreservedAnalyses::none());
    }
    let held = Held::of(context, layout, function, analyses, true).with_bounded(context, layout, function, analyses);
    // What is known through memory is the manager's, as every other client
    // reads it.
    let through = analyses.get::<llrm_analysis::manager::ThroughMemory>(context, layout, function);
    let facts = Result::as_ref(&*through).map_err(String::clone)?;
    let decisions = _decisions(&held.unit(context, layout, function, analyses.outer()), facts)?;
```

and `jumpthread`, which rebuilds blocks with the logging primitives, then invalidates everything and runs `fold`, `algebraic`, `decide` and `dead` to re-ask every fact ([`jumpthread.rs#L227-L246`](https://github.com/ali-mosavian/llrm/blob/269fdf3d11aedeeea6e933b9181c922dc1b17fe8/crates/opt/llrm-transforms/src/jumpthread.rs#L227-L246)):

```rust
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if !threaded(unit.context, unit.layout, unit.function, &outer, self.size, self.correlated, self.states) {
            return PreservedAnalyses::all();
        }
        // What the copies know of their state: a product by a constant, a
        // compare of one. Only where something was copied.
        analyses.invalidate(unit.function, &PreservedAnalyses::none());
        for pass in [
            &mut crate::fold::Fold as &mut dyn FunctionPass,
            &mut crate::algebraic::Algebraic { size: false },
            &mut crate::decide::Decide,
            &mut crate::dead::Dead,
        ] {
            let kept = pass.run(unit, analyses);
            analyses.invalidate(unit.function, &kept);
        }
        PreservedAnalyses::none()
    }
```

**Mirror or deviate.** Mirror the placement: whole-function solves at named positions, per-query facts with caps in the frequent passes. Deviate in the mechanism: LLVM declares required and preserved sets per pass (`getAnalysisUsage`, `PreservedAnalyses`); llrm's passes ask by code, and its manager already keeps a result across an edit by a mark and a change log (`Analyses::get`, `Touched`), which is finer than a preserved set. The position list replaces the declaration; the change log stays the invalidation. Reason and number: the analysis manager's mechanics are not the cost (invalidation 2.85 M against LLVM's 2.63 M, [analysis-manager](../comparison/llvm/analysis-manager.md)); who asks, and how often, is.

## What the measurements say

All on `d_faces` -O2 (`-m32 -march=i486 -fno-inline-functions`), origin/main as stated, sources in [#1994](https://github.com/ali-mosavian/llrm/issues/1994).

| measurement | result |
|---|---|
| analysis computes at function level | 2,742 (631 M own); 545 (20%, 41.3 M, 1.4% of the compile) on a body version already computed for; 1,965 follow an edit |
| the family Held, ThroughMemory, decide's facts | 403 M own (12.8%); decide visits 574 branches and decides none |
| idle runs of decide, hoist, indvars | 86 M own (3.0%): none has a cheap "would have said no" check left except hoist's, which exists |
| through-memory (attrs-budgets, S1 step 0, PR #2054) | enables 24.8% of `fold`'s edits (matmul -O3 up to 97.7%) and none of `decide`'s (33 M wasted); `fold` without it makes 96 more edits in the first run and 240 fewer in total |
| bounded (same) | enables nothing in six passes at -O2 (one matmul -O2 `algebraic` case, 5.5%) and costs 6-10% of `fold`, `hoist`, `algebraic` |
| `jumpthread` | its edits are logged; after it threads it invalidates everything, and 101.5 M of its 112.5 M is the `fold`, `algebraic`, `decide`, `dead` that re-ask every fact (one function) |

### The placement probe

The harness (`LLRM_SOLVE_AT`, `LLRM_PLACE_FACTS`; throwaway branch) names the steps that solve a family; elsewhere a pass is handed the held result if the manager has one, else the empty one. With every step named the objects are byte-identical (0 of 803 differ). Objects that differ, of 803 (vsgcc, run fixtures, QCport; no bench):

| family solved only at | `d_faces` instructions | saving | objects differ |
|---|--:|--:|--:|
| (all steps) | 2,736 M | | 0 |
| bounded, every `fold` | 2,641.8 M | 3.4% | 14 (1.7%) |
| bounded, `r01-fold` | 2,619.4 M | 4.3% | 35 (4.4%) |
| through-memory, every `fold` | 2,637.0 M | 3.6% | 123 (15.3%) |
| through-memory, `r01-fold` | 2,584.8 M | 5.5% | 161 (20.0%) |
| through-memory and bounded, `r01-fold` | 2,488.4 M | 9.0% | 188 (23.4%) |

`registers`, `annotated` and `exposed-frames` cannot be placed: withheld from `fold` the `d_faces` object doubles (8,068 to 15,320 bytes); they are the cheap many and stay per version. Through-memory away from `decide` is already its own change (#2061). "Objects differ" is the instrument, not the verdict: the rows tool (bytes and executed work against min(gcc, clang), worst program beside geomean) decides, and no row may get worse.

## The rule, and why not asking lazily at the rewrite site

**The rule.** A whole-function fact is solved at every pass whose edits consume it, and at no other: the set a pass reads is the pass's declared requirement, LLVM's `getAnalysisUsage` required set, and a position is where a consumer sits, not a tuned list. The probe states it from the consumer side: bounded is consumed by `fold` and `algebraic` (`tile`'s object is byte-identical when bounded is solved at `fold,algebraic`, and 3 of 803 corpus objects differ), through-memory by `fold` and not by `decide` (0 of its edits).

**Why not lazy.** A pass that asks "when it needs it" would make placement fall out of the passes' own asks, and for per-query facts that is the design (LLVM's `AAManager`, `computeKnownBits` to depth 6: asked at the site, capped, no whole-function state). It does not work for the facts here that are solves:

1. *A solve answers for one body version, and a pass edits as it goes.* The ask after an edit is a rebuild. `fold` builds `ThroughMemory` once per body version (5 builds on the `mem` ladder at N=1600), each one a walk of every load (4,832 provider queries); that is 531 M of `fold`'s 600 M, and the manager's "compute on first ask" is what produces 2.8 computations per pass run against LLVM's 0.5. A lazy ask at the site keeps the rebuild per edit; placement removes the ask.
2. *The pass cannot know that it will not need the answer until it has it.* `decide` asks the facts at 574 branch visits and decides none; the facts are what decides. A guard on "a load is in the condition's cone" was built and measured: `ThroughMemory` is asked by `fold` earlier in the same pipeline, so decide reads a cached result and the guard saved nothing (+0.6% of decide, +0.03% of the compile; [compile-time-spec section 8](compile-time-spec.md), "Decide's facts only where a branch needs them"). Only a per-query form of the fact, which does not exist for `ThroughMemory`, would make the ask cheap.
3. *Consumers read most of a solve.* `Annotated` is asked for 87% of what `point_values` solves ([redesign-plan](redesign-plan.md), 2.1); a per-query form saves what the pass does not ask, which is little.

What is not measured, and decides between the two: the cost of a capped per-load walk for `ThroughMemory` at the site (LLVM's `memdep-block-scan-limit` 100) against the whole build, per ask. That is the prototype with the kill rule "asks times cost per ask at least the whole build" ([redesign-plan](redesign-plan.md), 3.1). Until it exists the placement and the lazy form are not alternatives: the facts that have a per-query form are asked lazily with a cap; the solves are placed.

## Design

**Positions.** A solver position is a step label (`r01-fold`) or a pass. Which: the first `fold` of round 1 and the `fold` after `gvn` are the candidates; step 0's per-(pass, round) counts of fact-enabled rewrites choose them. The number of positions per function is bounded (the study's rule: three or fewer cover 90% of fact-enabled rewrites).

**Between positions.** A frequent pass reads the held result if the manager has it and it is current (the mark equals the result's), else the conservative one. It never triggers a whole-function solve. Counters: whole-function solves per function at most the positions; `analysis_computations / pass_runs` at most 0.5.

**The asks that remain.** Updated from the change log, not rebuilt: `Registers` (the manager has `update`; `indvars`' `Standing` re-derives it after every widening: 26 M, 0.9%), `Counted`, `Bounded` (incremental already). Asked at the site: `decide`'s facts for the cone of the condition it looks at (`constant_cycles::propagated(.., only: cone)` exists). Logged: `jumpthread` copies a block, retargets edges and rewrites phi inputs through the editor, so that its edits are `Change` records and the manager updates instead of dropping everything.

**Oracle.** A withheld result must not trip the replay oracle: `LLRM_CHECK_REPLAY` asserts a held `Bounds` equals a fresh solve and reports an empty one as stale (479 of the first probe's 803 "differences" were that panic). A withheld result is marked, and the replay check skips a marked one. The mechanism lands identical first (positions = every step), then the placement changes under the rows rule.

**Kill rule** (S1, fixed before the build): under 90% of fact-enabled rewrites at three or fewer positions; under 10% of `d_faces` -O2 saved; computes per run above 1.0; or a row crosses above min(gcc, clang) that one more position does not recover. The probe so far: the best configuration saves 9.0%, just under, with 23% of objects changed, so as a blanket lever it is killed; what survives is narrow (bounded at `fold`, through-memory away from `decide`) and goes to the rows tool one family at a time.

## Open

- Rows for bounded at `fold` and `algebraic`: done (see the TL;DR); at `fold` alone killed.
- Whether a withheld fact that is empty is worse than conservative for a consumer that assumes a fact exists (the probe's 15-23% differing objects).
- The alias tower (`Annotated`, `Pointers`) is untested by step 0.
