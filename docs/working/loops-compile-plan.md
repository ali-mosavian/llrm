# Loops compile-time bucket: dated plan to 1.0x of the pin

Owner w-3. Ledger command (docs/working/ledger-command.md): qbq-llrm render/d_faces.c, `-m32 -mabi=sysv -march=i486 -O2`, callgrind, dist build of a206759f8. Bucket: 160.5 M ours, 86.8 M pin (1.85x, +73.6 M). Callee view below is inclusive M on that run; "pin" is the pin function(s) the step mirrors.

| # | Where (ours, M) | Pin mirror | Cause | Removes | PR date |
|---|---|---|---|---|---|
| L1 | Licm 48.0 (pin 14.0): `MemorySsa` get x11 = 35.1, `sink_and_hoist` 9.2 | `MemorySSAAnalysis` / `buildMemorySSA` ([L1522](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/MemorySSA.cpp#L1522)): 3.0 M for the compile | per-build cost: CallFootprints, Annotated, `Accesses::plain` (docs/working/accesses-basicaa-plan.md) | ~25 | S1b 11-13, S2 11-20, S3 11-27 |
| L2 | IndVars 45.0 (pin 23.3): `eliminate_iv_user` 24.9 x533: `evaluate_predicate_at` 14.3 (pin's `evaluatePredicateAt` 13.0 in clang's own run: parity), `get_scev` 6.8 x466, `get_loop_invariant_predicate` 3.1 (pin 2.6); `ExitOpt` 6.9; the rest of `simplify_users_of_iv` 4.4 | `SimplifyIndvar::simplifyUsers` ([IndVarSimplify total 23.3, simplifyUsers 15.4](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp)), `optimizeLoopExits` | the predicate queries match the pin's cost; the gap is `get_scev` (SCEV nodes made again where the pin's SE survives the passes between), `ExitOpt` and the per-user overhead | ~8 (was ~18 before the pin's evaluatePredicateAt was read) | 11-13: count SCEV nodes made per pass on the ledger command, keep the SE across the loop passes (w-5 reviews) |
| L3 | LCSSA 7.0 (pin 0.8): `closed_in` 6.0: `formed_with` 4.8 (`outside_reads` 2.2, `lcssamerges::closed` 1.6) | `formLCSSAForInstructions` (LCSSA.cpp) | users scanned per instruction, not per use list | ~5 | 11-20 |
| L4 | LoopLoadElim 7.5 (pin 2.3): `get_backedge_taken_count` 2.5, `AccessAnalysis::can_check...` 1.4, `are_deps_safe` 0.9 | `LoopLoadEliminationPass` / `LoopAccessInfo` | trip count and access analysis made per loop, not shared with IndVars' SE | ~3 | 12-04 |
| L5 | LoopIdioms 4.7 + Merge 1.3 + LoopIdiom 5.1 = 11.1 (pin 4.1) | `LoopIdiomRecognize` | two implementations: the llrm-only `loopidioms` still runs beside the translated `loopidiom::pass` | ~6 | 11-27 (retire `loopidioms` where `loopidiom` covers it; objects compared) |
| L6 | LoopSimplify 4.1 (pin 1.0 + SimplifyCFG 1.0) | `LoopSimplify` | `_simplified` re-analyses; loops found per call | ~2 | 12-04 |
| L7 | LSR 17.1 (pin 12.8): `generate_all_reuse_formulae` 5.0, `implement_solution` 3.3, `IvUsers::add_users_if_interesting` 2.7 | `LSRInstance::GenerateAllReuseFormulae`, `Rewrite`, `IVUsers` | formula sets cloned per use | ~3 | 12-11 |
| L8 | the rest: unroll 10.5 (pin 9.1), deletion 6.3 (7.3), rotate 2.9 (7.8): at or below the pin | - | - | 0 | - |

Sum of the steps: about 52 M of the 73.6 M excess, so the bucket lands near 108 M (1.25x). The last ~22 M is not attributed to a callee yet: L9, 12-18, re-measure after L1 to L3 on the ledger command and plan the next steps from the callee view; the goal stays 1.0x.

Each step is one PR with its cost measured on the ledger command (dist, before and after, same target dir) and objects compared (objsweep, QCport) where the change can reach output; none is in this PR.

## From about 1.25x to 1.0x (not dated yet)

After L1 to L7 the bucket is near 108 M against 86.8 M. What is left, with what each needs before it can get a date:

| # | Where (ours after L1 to L7, est. M; pin) | Needs |
|---|---|---|
| R1 | IndVars ~37 (pin 23.3): `ExitOpt` 6.9 (pin's `optimizeLoopExits` ~3.9), `simplify_users_of_iv` outside `eliminate_iv_user` 4.4, `get_scev` | measurement: SCEV nodes made per pass on the ledger command, ours against the pin's `-stats`-less count (callee view of `createSCEV`) |
| R2 | cold `ScalarEvolution` starts: `delete_loops` x60, `ScevSession::within` x51, unroll x37, LoopDeletion x32, IndVars x19 (the pin keeps one SE for the loop pipeline) | blocker: which passes drop `Scevs` between the loop passes (`PreservedAnalyses`); measurement: nodes made after each |
| R3 | Licm ~23 (pin 14.0): `sink_and_hoist` 9.2 x62 against `hoistRegion` 10.6 in the pin | measurement: `can_sink_or_hoist_inst` x914 (3.5) against the pin's `canSinkOrHoistInst` 8.9 on the same loops |
| R4 | LoopLoadElim ~4.5 (pin 2.3), LCSSA ~2 (0.8), LoopSimplify ~2 (1.0 + 1.0), LSR ~14 (12.8) | after L3, L4, L6, L7 are measured |
| R5 | rotate 2.9, deletion 6.3, unroll 10.5 against the pin's 7.8, 7.3, 9.1 | none: at or below the pin; keep out of the way |
