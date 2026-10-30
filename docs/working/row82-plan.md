# Row 82, Control Flow Optimizer: translation plan

Owner w-3. Pin: [BranchFolding.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp) at d1106deb71cc. Ours: `jumps/tailmerge.rs`, `jumps/branchfold.rs`, `jumps.rs::optimized`. Order below is the order the pin runs; dates are the day the code lands.

Blocker B1 (D20): ours folds branches after placement has laid the blocks out and never redoes the layout; the pin's block moves (`moveAfter`/`moveBefore`) change the layout and LLVM then reruns its layout-dependent rules. Needs a layout-redo hook in `jumps.rs::optimized` (re-run `Exit::of` and the placement-dependent rules after a move). Lands with step 6, 11-06.

| # | Pin function | Status | Blocker | Date |
|---|---|---|---|---|
| 1 | [`OptimizeFunction` L237](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L237): TailMerge, OptimizeBranches, HoistCommonCode, repeated while any changed | partial: ours runs each once | none; loop the three until no change | 11-13 |
| 2 | `TailMergeBlocks` [L1109](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1109) with `TryTailMergeBlocks` L981, `mergeCommonTails` L902, `SplitMBBAt` L485 | translated (`tailmerge.rs`, jumps-tailmerge-check.md) | none | done |
| 3 | `OptimizeBlock` [L1426](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1426): empty and branch-only blocks bypassed L1451; one-way conditional L1503; jump to next dropped / `jcc;jmp` reversed L1551-1585 | translated | none | done |
| 4 | L1515: previous block falls through into a block with no other predecessor, contents merged into it | not translated | none (our blocks are LIR; check `_prune` does not already cover it) | 11-04 |
| 5 | L1586 identical conditional branch; L1685 one-block loop reversal | translated (#3691) | none | done |
| 6 | L1601 no-successor block moved to the end; L1803 and L1868 fall-through creating moves (`moveAfter`/`moveBefore`) | not translated | B1 | 11-06 |
| 7 | L1702 forwarding across a branch-only block (`IsBranchOnlyBlock` L1339, `IsBetterFallthrough` L1349) | not translated | none for the forwarding itself; the moves it enables need B1 | 11-06 |
| 8 | `HoistCommonCode` [L1910](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/BranchFolding.cpp#L1910) / `HoistCommonCodeInSuccs` L2043 (`findFalseBlock` L1920, `addRegAndItsAliases` L1929) | partial: only the inserted-move case (`hoist_common`) | register def/use sets and alias walk over LIR operands: reuse `liveness::effect`; flags clobber check | 11-13 |
| 9 | `RemoveDeadBlock` L213, `setCommonTailEdgeWeights` L1259 | dead blocks pruned in `_prune`; edge weights follow `BlockOdds` | none | 11-04 (check only) |
| 10 | EH-scope checks L1434, debug-info copies L1370-1401, pseudo-probe L376-417 | N/A (no EH/debug/probes in the DOS path) | none | n/a |

Each step: fail-first test on the symptom, objsweep (568 objects) and QCport 260 objects for bytes, 70-program bench for taken branches and clocks; a step lands once it builds and passes the runner's gate, numbers go in the PR.
