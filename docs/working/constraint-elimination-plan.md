# ConstraintElimination: translation plan (w-5, 2026-10-25)

Owner w-5, assigned by the coordinator on w4's finding. Pin: [ConstraintElimination.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp) (2,711 lines) and [ConstraintSystem.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp) (311 lines) at d1106deb71cc. Ours has a `Missing` registry entry (`parity.rs`: `constraint-elimination`, pass #35 of `default<O2>`, after Reassociate and before LoopSimplify/LCSSA/LICM).

## Why

w4's loop (`bench_scroll`, third loop of `bench/scroll/scroll.nib`, `/home/alim/scratch/w4-scroll/c/un.ll`): the pin's `constraint-elimination` alone turns Nib's `j <u 1920` check into `br i1 true` (header phi `{1919,+,-1}<nsw>` gives `j <= 1919`, the header test gives `j >= 0`); instcombine, simplifycfg, jump-threading and CVP do not. Then LoopIdiom sees a clean copy loop and forms one memmove: scroll.nib m16 -O2 executes 602,661 instructions at tip against 170,762 on the old side (w4 re-measures the baseline before a ratio is quoted). The slot table's "ablation 0 of 12 IRs" came from a corpus with no such loop.

## Order: a vertical slice first (step 1 of 5), then the pin's file order

The coordinator's reorder: a shipped m16 program loses x3.5 until the slice lands, so step 1 is a thin vertical cut by **2026-11-20** that already removes scroll's check, translated from the pin as it is and not a special case, so every later step only widens it: the real `ConstraintSystem` (CE-1, #4457), `decompose` for one variable plus a constant (`j <u C`, `sgt j, -1`), condition facts from branches only (`State::addInfoFor`'s branch part), the dominator-tree walk (`eliminateConstraints`'s loop, `FactOrCheck` ordering), `checkAndReplaceCondition` for an `icmp` use, and the slot in the pipeline (#35, after Reassociate). Not in the slice, with the step that adds each (all of them the pin's own code, none a rewrite): assumes and intrinsics (CE-3), header-induction bounds and monotonicity (CE-3), GEP offsets, shl/mul/sext/zext/trunc/min/max/or-disjoint decomposition and multi-variable rows (CE-2), flag strengthening, min/max and overflow-intrinsic checks (CE-4). Gate for the slice: scroll.nib m16 -O2 executed instructions before and after on dist (w4's baseline), 0 wrong output over the corpus, and a DEVIATE row for everything not yet translated (CE-S1: only the slice's decompositions and facts, exit 12-18, w-5).

## Order (the pin's file order)

| step | what | pin lines | date |
|---|---|--:|---|
| CE-1 (#4457, done) | `ConstraintSystem` (rows, `addVariableRow`, `isConditionImplied`/`getLastConstraint`, Fourier-Motzkin elimination `eliminateUsingFM`, the unsigned/signed row split), as a function of integer rows, tested against the header's contract | 311 + 180 header | 2026-11-06 |
| slice | the vertical cut above, on CE-1 | | 2026-11-20 |
| CE-2 | `ConstraintInfo` and the decomposition (`decompose`: add, sub, mul by constant, shl, zext, sext, trunc, GEP offsets `collectOffsets`, min/max, `or disjoint`; `getConstraint`, `addFact`, `doesHold`) | 300-1120 | 2026-11-20 |
| CE-3 | `State`: the dominator-tree walk, `addInfoFor` (conditions, assumes, intrinsics, header inductions `addBoundsForHeaderInductions`/`addInfoForInductions`, `getMonotonicityInfo`), `FactOrCheck` ordering | 1120-1720 | 2026-12-04 |
| CE-4 | `eliminateConstraints` (`checkAndReplaceCondition`, `checkAndReplaceMinMax`, `checkAndReplaceCmp`, the `tryToStrengthenFlags` of `nuw`/`nsw`/`samesign`) and the `ConstraintEliminationPass::run`; registry slot after Reassociate | 1960-2700 | 2026-12-18 |

The reproducer dump (`ReproducerEntry`, lines 1726-1960) is debug output and not translated (a named DEVIATE).

## Oracle

`opt -passes='function(constraint-elimination)' FILE` on each IR the d_faces `--dump` and w4's `un.ll`/`bc.ll` give; the corpus of the SCEV oracle (`crates/opt/llrm-analysis/corpus`) for the rest; the pass's output compared as the other passes' tests are (`tools/difftest`). Fail-first test for the slot: w4's loop through the whole pipeline ends in `memmove` (m16: `std; rep movsd`).

## Cost

The pass is 2.7 K lines of per-instruction work on the dominator walk; the pin's takes 2.67 M Ir in the d_faces compile (callgrind on `clang -O2`, `ConstraintEliminationPass::run`, 0.67% of the compile). The slot is a compile-time cost as well as a code win; both are measured on a dist pair when it lands.
