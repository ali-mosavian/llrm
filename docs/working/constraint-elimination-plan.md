# ConstraintElimination: translation plan (w-5, 2026-10-25)

Owner w-5, assigned by the coordinator on w4's finding. Pin: [ConstraintElimination.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/ConstraintElimination.cpp) (2,711 lines) and [ConstraintSystem.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstraintSystem.cpp) (311 lines) at d1106deb71cc. Ours has a `Missing` registry entry (`parity.rs`: `constraint-elimination`, pass #35 of `default<O2>`, after Reassociate and before LoopSimplify/LCSSA/LICM).

## Why

w4's loop (`bench_scroll`, third loop of `bench/scroll/scroll.nib`, w4's reduced `un.ll`): the pin's `constraint-elimination` alone turns Nib's `j <u 1920` check into `br i1 true` (header phi `{1919,+,-1}<nsw>` gives `j <= 1919`, the header test gives `j >= 0`); instcombine, simplifycfg, jump-threading and CVP do not. Then LoopIdiom sees a clean copy loop and forms one memmove: scroll.nib m16 -O2 executes 602,661 instructions at tip against 170,762 on the old side (w4 re-measures the baseline before a ratio is quoted). The slot table's "ablation 0 of 12 IRs" came from a corpus with no such loop.

## State: translated, not in the pipeline

`constraintelim.rs` (2,975 lines, pin-linked) and llrm-mir `constraintsystem.rs` (#2324) are in main; `parity.rs` still says `Missing`, no slot runs the pass. Its DEVIATE 5 is the only thing scroll's loop lacks: `addBoundsForHeaderInductions`, `addInfoForInductions` and `getMonotonicityInfo` need ScalarEvolution. (An earlier version of this plan assumed no translation exists; CE-1 #4457 duplicated #2324 and is closed.)

## Steps

| step | what | date |
|---|---|---|
| slice | induction facts from SCEV (closes DEVIATE 5), the pass at #35 after Reassociate, w4's loop ends in `memmove`; gate: scroll.nib m16 -O2 executed instructions before/after on dist, 0 wrong output over the corpus | 2026-11-20 |
| CE-2 | memory-access facts (DEVIATE 6, needs `getBaseObjectSize`), remaining `cecompare.py` diffs | 2026-12-04 |
| CE-3 | compile-cost: dist pair against the pin's 2.67 M Ir on d_faces | 2026-12-18 |

The reproducer dump is debug output and not translated (DEVIATE 3).

## Oracle

`opt -passes='function(constraint-elimination)' FILE` on each IR the d_faces `--dump` and w4's `un.ll`/`bc.ll` give; the corpus of the SCEV oracle (`crates/opt/llrm-analysis/corpus`) for the rest; the pass's output compared as the other passes' tests are (`tools/difftest`). Fail-first test for the slot: w4's loop through the whole pipeline ends in `memmove` (m16: `std; rep movsd`).

## Cost

The pass is 2.7 K lines of per-instruction work on the dominator walk; the pin's takes 2.67 M Ir in the d_faces compile (callgrind on `clang -O2`, `ConstraintEliminationPass::run`, 0.67% of the compile). The slot is a compile-time cost as well as a code win; both are measured on a dist pair when it lands.
