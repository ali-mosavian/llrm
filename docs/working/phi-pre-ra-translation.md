# Before greedy: PHIElimination, TwoAddressInstruction and the order, against the pin

Pin d1106deb71cc. Read from the sources (what I read is said per row); nothing here is a measurement. Step 0 (w-6): 0 of 4498 functions reach the old allocator, so no phi reaches greedy today.

## PHIElimination (`phielimination.rs`, `lowered`)

| pin | ours | state |
|---|---|---|
| [`SplitPHIEdges`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L808) (live out past the phis, loop exits) | `edges_to_split` | same (header says translated; I did not diff the conditions line by line) |
| [`LowerPHINode`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L367): one temporary per phi, `dest = copy temp` at the top, `temp = copy src` per predecessor (one copy per predecessor block) | the same shape, `phielim::_copy` | same |
| reuse of an identical phi's temporary when every edge into the block is critical (`LoweredPHIs`) | `lowered_phis`, keyed by the sorted incoming pairs | same |
| temporary created in the result's class ([L414-L415](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIElimination.cpp#L414)) | a fresh vreg with no class; the coalescer's palette comes from use | differs (structural: LIR values have no class field). Effect shown on render/dl: the segment-class result and the temporary refuse to share, 14 instructions. A local fix is DEVIATE C10 in `coalesce.rs` (branch `mir-undef/phi-temp-class`, not landed) |
| copy insert point [`findPHICopyInsertPoint`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/PHIEliminationUtils.cpp): before the first terminator, or after the last def / before the call for an EH-pad or `INLINEASM_BR` successor | `_before_the_terminator` | same for the plain case; the EH-pad and `INLINEASM_BR` cases: not compared |
| `IMPLICIT_DEF` / undef operands (`allPhiOperandsUndefined`, `ImpDefs`) | `isImplicitlyDefined`, `allPhiOperandsUndefined`, `SrcUndef`, `ImpDefs` | same (an `implicit_def` of the result, of the temporary on an undefined edge; the dead ones go) | same (P2; PI-2 of ProcessImplicitDefs retired with it) |
| `LiveVariables` / `LiveIntervals` updates, kill flags | none | n/a (LIR has no kill flags) |
| a phi with an edge from outside the function | left alone | differs, P1 |
| x87 phis | left alone without RFP, lowered like any other under RFP | differs, P4 |

So C10 is the only difference that changes classes, but it is not the only difference in the pass. The class itself cannot be set in the lowering until a value has one.

## TwoAddressInstruction (`twoaddr.rs`)

`twoaddr.rs` is not a translation of the pass; its "See also" cites llvmorg-18.1.8, not the pin.

| pin | ours | state |
|---|---|---|
| tied operands collected from the instruction descriptors ([`collectTiedOperands` L1554](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1554), [`processTiedPairs` L1593](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1593)) | `ties`: four operations (`Binary`, `Unary`, `Funnel`, `Select`) read their destination | differs |
| [`tryInstructionCommute` L1285](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1285), [`isProfitableToCommute` L648](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L648) | `_commuted`, `_less`: add, and, or, xor, imul; its own order of reasons, plus the physical-register rule of `isProfitableToCommute` | differs |
| [`convertInstTo3Addr` L801](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L801) (x86 `convertToThreeAddress`: `add` to `lea`) | none here (selection makes LEAs itself) | missing at this stage |
| [`rescheduleMIBelowKill` L928](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L928), [`rescheduleKillAboveMI` L1116](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1116) | none | missing |
| source/destination hint maps (`scanUses` L850, `processCopy` L902, `isRevCopyChain` L320, `findOnlyInterestingUse` L499) | `_copy_destinations` graph; update in place (`_reused`) | differs |
| [`processStatepoint` L1792](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1792), [`eliminateRegSequence` L2035](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L2035) | none | n/a (no statepoints or `REG_SEQUENCE` in LIR) |
| a memory destination computed in a fresh register | `_untied` | ours only |

## Order, `TargetPassConfig::addOptimizedRegAlloc` ([L1497](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L1497))

Ours from the `ISEL_DUMP=1` stage names on render/dl: FarIndirectCalls, OptimizePHIs, SharedConstants, MachineCSE, MachineSink, PhiElimination, PressureSink, FloatFold, FloatPrep, TwoAddress, Coalescer, RegAlloc, FloatAlloc, ParallelCopy, Peephole, LoopSlots, Scheduler, ControlFlow.

| pin | ours | state |
|---|---|---|
| DetectDeadLanes, InitUndef, ProcessImplicitDefs | none | n/a (no lanes, no implicit defs) |
| UnreachableMachineBlockElim, LiveVariables | none | n/a / missing: I did not check whether unreachable LIR blocks reach here |
| MachineLoopInfo, PHIElimination | MachineLoopInfo as an analysis, PhiElimination | same order |
| TwoAddressInstruction, RegisterCoalescer | TwoAddress, Coalescer | same order |
| RenameIndependentSubregs | none | n/a (no lanes) |
| MachineScheduler (pre-RA) | none before RegAlloc; `Scheduler` runs after | missing |
| greedy + VirtRegRewriter, StackSlotColoring | RegAlloc (the slot table groups them) | same order, as one pass |
| PostRewrite, MachineCopyPropagation, MachineLICM | `Peephole`, `LoopSlots`: not compared | not checked |
| none between PHIElimination and TwoAddress | PressureSink, FloatFold, FloatPrep | ours only |
| none between TwoAddress/coalescing and greedy | none | same |
