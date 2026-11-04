# SCEV per-loop cost: one persistent SE across the loop passes (w17, 2026-10-28, design only)

TL;DR: branches1024 (128 loops) computes each loop's trip count 7 times; the pin computes it 3 times. Target: 7 -> 3 by keeping `Scevs` across the loop passes. Reachable ~0.6 G of 9.6 G (6%), per w5's POC. No build yet.

## What exists
- `Scevs` (`llrm-analysis/src/scev/analysis.rs`): one `ScalarEvolution` per function in the manager; `take`/`put` hand it to a pass that edits while it keeps it current (`ScevSession::adopt`/`into_scev`).
- Only four passes keep it: `indvars.rs:324`, `canonfreeze.rs:73`, `loopdeletion.rs:1016`, `lsr_pass.rs:98` (only when it kept the CFG). Every other loop pass (looprotate, loopidiom, looploadelim, loopunrollpass, looppeel, exitopt, ...) returns `PreservedAnalyses::none()`/`all()` without `Scevs`, so the manager drops the SE and the next pass recomputes every trip count.
- The count is dropped in the pass too: `forget_touched_keeping` (`scev/forget.rs:129`) forgets the trip count of every loop an edit touches unless `keep_counts`; LLVM drops one only on `forgetLoop`, a deleted value or an explicit `forgetValue`. w5's POC (skip that forget): 7 -> 5 computes, objects identical.

## Plan (one PR, draft till done)
1. Audit each loop pass for what it edits and whether LLVM's pass preserves SCEV (`getLoopAnalysisUsage` / `PreservedAnalyses::preserve<ScalarEvolutionAnalysis>` in the pin): list in the PR body, pass by pass.
2. For each pass that the pin preserves SCEV through, return `.preserve::<Scevs>()` and make its own edits go through `ScevSession` (forget exactly what LLVM forgets: `forgetLoop`, `forgetValue`, `forgetBlockAndLoopDispositions`).
3. `forget_touched_keeping`: keep the trip count where LLVM would (operand replaced in place), as w5's POC did; this is a DEVIATE removal.
4. Work-count test: a function with N loops through the pipeline asserts `trip_count_computes <= 3 * N` (add a counter next to `module_runs()`); fail-first by reverting the preserve lines.

## Files
`llrm-analysis/src/scev/{analysis,forget}.rs`; the loop passes under `llrm-transforms/src/`: looprotate, loopidiom/pass, looploadelim, loopunrollpass, looppeel, exitopt, simplifyindvar; `parity.rs:513` lists `Scevs`.

## Risks
- Stale SCEV after an edit the pass did not forget: wrong code, not a crash. Mitigation: `LLRM_CHECK_*` replay (`ScevCache::consistent` compares with a fresh one) over the whole corpus and the torture lists; objects must stay byte-identical (any difference is a staleness or an ordering change and is explained or fixed).
- LSR runs in codegen prep on a fresh SE in the pin; do not widen its sharing.
- Preserving across a pass that changes the CFG: only `forget_expressions()` is safe there; keep it for those.
- Scope: w5 owns the known-bits excess (ValueTracking 1.16 G vs 0.35 G); not part of this PR.
