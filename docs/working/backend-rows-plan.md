# Backend rows to translate: 82, 34/83, 90, 84, 42 — w-3, plan note 2026-10-22

User order via oversight: translate and land the missing backend rows; a piece lands once it builds and passes the runner's gate; size, clock and compile numbers are stated in the PR and never block; a wrong output or compile failure found later is that day's first item. Pin: llvm-project `d1106deb71cc81016406a1917d0508d2df296266`. Rows and measured effects: [codegen-slot-table.md](codegen-slot-table.md), [tail-dup-placement.md](tail-dup-placement.md), [early-machinelicm.md](early-machinelicm.md). Each PR carries the row's ablation numbers (pin with and without the switch against ours, `tools/pairs/pinswitch.py`, clocks by `tools/pairs/bench_pin.py`), objects compared with `tools/pairs/objsweep.py`, and a fail-first test.

## Row 82, Control Flow Optimizer (BranchFolder): land 11-04

Pin order, `BranchFolder::OptimizeFunction` ([L237](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/BranchFolding.cpp#L237)): `TailMergeBlocks` L1109, then `OptimizeBranches` L1307 (`OptimizeBlock` L1426 over every block until nothing changes), then `HoistCommonCode` L1910 (`HoistCommonCodeInSuccs` L2043), repeated while any changed.

Ours: `jumps/tailmerge.rs` translates `TailMergeBlocks` ([check](../architecture/jumps-tailmerge-check.md)); `jumps/branchfold.rs` translates three rules of `OptimizeBlock` (empty and branch-only blocks bypassed L1451, a conditional with one way a jump L1503, a jump to the next block dropped / `jcc; jmp` reversed L1551). Not translated: D20 the block moves (`moveAfter`/`moveBefore`), the identical-terminator block L1586, the no-successor block moved to the end L1601, the single-block loop reversal L1685, the forwarding across a branch-only block L1702, the fall-through creating moves L1803 and L1868, and `HoistCommonCode`.

PRs: (1) the in-place rules (L1586, L1685, L1702) in `branchfold.rs`; (2) the moves with the layout redone as LLVM does after a change (D20); (3) `HoistCommonCode`. Each against the 70-program bench (taken branches, clocks) and objsweep.

## Rows 34 and 83, Early and late Tail Duplication, with STRISTMP-1: land 11-06

Pin, `TailDuplicator` ([TailDuplicator.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/TailDuplicator.cpp)): `tailDuplicateBlocks` L274 (per block `shouldTailDuplicate` L562, `canTailDuplicate` L825, `tailDuplicate` L859, the SSA updates `processPHI` L350 / `updateSuccessorsPHIs` L482 / `addSSAUpdateEntry` L335 for the early pass, `duplicateInstruction` L388, `appendCopies` L1067), `duplicateSimpleBB` L753 for blocks `isSimpleBB` L717 and `canCompletelyDuplicateBB` L737 allow, `tailDuplicateAndUpdate` L165, `removeDeadBlock` L1081. Early: before register allocation in SSA (`-disable-early-taildup`, pre-RA size limit 2 instructions); late: after allocation, no SSA (`-disable-tail-duplicate`).

Ours: only the layout-mode part inside `jumps/placement/taildup.rs` (`shouldTailDuplicate`, `canTailDuplicate`, `tailDuplicate` after allocation, called from placement). Not translated: the stand-alone late pass at its slot (before placement and the second copy propagation), the SSA early pass, `duplicateSimpleBB`, and the return-block case of STRISTMP-1 (main duplicates `ret` into both arms, ours does not: `shouldTailDuplicate`'s `HasIndirectbr`/return rule L562-716).

PRs: (1) the late stand-alone pass over the existing `taildup.rs` functions, `ret` blocks included (STRISTMP-1); (2) the early pass over the LIR's phis with an SSA update, behind the same `shouldTailDuplicate`.

## Row 90, Branch Probability Basic Block Placement: land 11-06

Pin: `MachineBlockPlacement` per [jumps-placement-check.md](../architecture/jumps-placement-check.md) (stages 1 to 5, `buildCFGChains` L2799, `buildChain` L1920, `selectBestSuccessor` L1665, `buildLoopChains` L2706, `rotateLoop` L2423, `optimizeBranches` L2964, `alignBlocks` L3003). Ours translates the stages with five stated deviations (D2 gcc-trace successor choice instead of `selectBestSuccessor`'s probabilities, D3 duplication by bytes, D4 loop headers after latches, D5 the merge before and after, D6 llrm's `Frequency`).

D2 is already retired in the tree (`placement.rs::best_successor` = `selectBestSuccessor` L1665; the check doc lists the rest as D3, D4, D5, D6, D22).

Measured gap (bench, 486 model, same IR): ours has 1.5% more taken branches at -O2 and 1.1% at -O3 than the pin, 19% more executed `jmp` at -O2; the pass is worth 0.84% of the pin's clocks. The largest single loop, nbody_single (16012 against 12002 taken), is not placement: the pin's LSR gives the inner IV a negative bias, so its inner tests are `cmpl $-24` (50/50 edges) where ours tests `test ecx,ecx` (zero heuristic, 37.5%); MachineSink then breaks that edge as the pin's `isWorthBreakingCriticalEdge` does (probability under 40%) and the x87 constants land in an edge block that placement puts between two hot blocks. The cause is LSR's choice of IV (w-5).

## Row 84, Machine Copy Propagation run 2: land 11-06

Pin: `MachineCopyPropagation::run` L1619 (`forwardCopyPropagateBlock` L926, `propagateDefs` L1116, `backwardCopyPropagateBlock` L1184, `eliminateSpillageCopies` L1316 only with the spill option) runs twice: after the post-RA scheduler region and again after tail duplication. Ours: `machinecp.rs` runs once as a sub-pass of `peephole`. PR: one slot after the late tail duplication, the same code; ablation `-disable-copyprop` (+122 instructions at -O2 on the pin).

## Row 42, Early MachineLICM, pressure-gated: land 11-06

Pin ([MachineLICM.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineLICM.cpp)), pre-RA `early-machinelicm` before MachineCSE: `run` L369, `HoistRegionPostRA` is the post-RA variant (row 72, N/A), `HoistOutOfLoop` L827 (walks the loop's dominator tree: `InitRegPressure` L933, `UpdateRegPressure` L952, `calcRegisterCost` L970), per instruction `IsLICMCandidate` L1098, `IsLoopInvariantInst` L1133, `IsProfitableToHoist` L1273 (`HasLoopPHIUse` L1144, `IsCheapInstruction` L1210, `CanCauseHighRegPressure` L1235, `UpdateBackTraceRegPressure` L1259, `HasHighOperandLatency` L1178), `ExtractHoistableLoad` L1400, `Hoist` L1615 with CSE (`LookForDuplicate` L1512, `EliminateCSE` L1525, `MayCSE` L1590), `getOrCreatePreheader` L1709, `isTgtHotterThanSrc` L1730.

Ours: nothing at this stage; the mid-end `licm` and `loopslots.rs` (reloads). PRs: (1) candidates, invariance and `Hoist` into the preheader without the pressure gate; (2) the register-pressure gate (`InitRegPressure` .. `CanCauseHighRegPressure`, limits from the target's register classes); (3) the CSE on hoist. Lands as a pre-RA LIR pass beside `precse.rs`.

## Order

82 (11-04), then 34/83, 84, 90, 42 (11-06). The compile-cost items (LCSSA leftovers #3683, LSR callees) follow.
