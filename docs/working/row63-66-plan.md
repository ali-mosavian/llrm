# Rows 63 (RegisterCoalescer) and 66 (RAGreedy): translation plan

Owner: splitkit-port. Pin d1106deb71cc. Permalink base: `https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/` + file + `#L<n>`. Status: T translated, D under a named DEVIATE, - not translated. Dates marked (p) are proposals awaiting oversight; the rest are accepted (coordinator, 10-22).

## Evidence for the DEVIATE exits

The pin's own `-stats` and MIR on our final IR (`--dump`, stage `*-switchlower.ll`, then the assert `llc -O2` for i486 or i386-code16), 272 runs: vsgcc 71 programs at m32 -O2 and -Os, QCport 65 modules at m16 -O2 and -Os. 43 runs failed (llc nonzero or no dump: x_funcptr, x_switch at m32; 20 QCport modules at m16), so the counts are of the rest. The pin's pipeline, not ours: it says where the pin's coalescer meets the construct, not what our output loses. Script `coalstats.py`, rows `coalstats.jsonl`.

| Construct (DEVIATE) | m32 (142 runs) | m16 (130 runs) | Reading |
|---|---|---|---|
| COPY with a sub-register index (C1) | 260 in 83 | 5316 in 88 | fires |
| dead lane conflicts tested / resolved (C1) | 13 / 7 in 10 | 122 / 96 in 43 | fires |
| COPY with a physical register (C2) | 1131 in 139 | 3256 in 88 | fires; ours are `requires`/`delivers` constraints |
| IMPLICIT_DEF before the coalescer (C3) | 0 | 8 in 4 | rare |
| `undef` operands (C6) | 30 in 20 | 1530 in 72 | fires (comes with C1) |
| terminal rule (C7) | - | - | cannot fire: `-terminal-rule` is off by default (RegisterCoalescer.cpp#L82) |
| debug values | - | - | cannot fire: no `-g` in the corpus, LIR has no DBG_VALUE |
| interval joins, cross-class joins | 5409, 759 | 15252, 6471 | what the translation does today |
| commutes, copies extended, remats | 17, 3, 557 | 8, 0, 2263 | C4 pieces now translated |

## Row 63, RegisterCoalescer.cpp, in the pin's order

| Pin function | Line | Status | Blocker and date |
|---|---|---|---|
| isSplitEdge | 453 | D | with the terminal rule (C7): not run |
| CoalescerPair::setRegisters / flip / isCoalescable | 464 / 554 / 563 | T (`pair.rs`) | |
| eliminateDeadDefs, LRE_WillEraseInstruction | 613, 623 | D (C5) | translate: coalescer uses `LiveRangeEdit::eliminateDeadDefs` (greedy/liverangeedit.rs): 11-06 |
| adjustCopiesBackFrom | 628 | T | |
| hasOtherReachingDefs | 770 | T | |
| addSegmentsWithValNo | 798 | T (inline in removeCopyByCommutingDef) | |
| removeCopyByCommutingDef | 823 | T | |
| removePartialRedundancy | 1115 | T | |
| reMaterializeDef | 1298 | T (C10, C11 deviate) | |
| eliminateUndefCopy | 1742 | D (C3) | IMPLICIT_DEF in LIR (w-1 rows 59/61): date from w-1 by 10-27; translate 2 days after it lands |
| addUndefFlag | 1865 | D (C6) | undef operand flags on LIR operands, with C1 below |
| updateRegDefsUses | 1891 | T (no undef flags) | |
| canJoinPhys | 1993 | D (C2) | physical-register live ranges in LiveIntervals: see C2 below |
| copyValueUndefInPredecessors | 2011 | D (C1) | subranges |
| setUndefOnPrunedSubRegUses | 2025 | D (C1) | subranges |
| joinCopy | 2054 | T | |
| joinReservedPhysReg | 2314 | D (C2) | physical-register live ranges |
| JoinVals::computeWriteLanes | 2746 | D (C1) | lane masks |
| JoinVals::followCopyChain, valuesIdentical | 2761, 2813 | T | |
| JoinVals::analyzeValue | 2837 | T, one lane | lanes |
| JoinVals::computeAssignment, mapValues | 3117, 3154 | T | |
| JoinVals::taintExtent, usesLanes | 3166, 3206 | D (C1) | lanes |
| JoinVals::resolveConflicts | 3222 | T, no unresolved | lanes |
| JoinVals::isPrunedValue, pruneValues | 3290, 3305 | T | |
| isLiveThrough | 3373 | D (C1) | with pruneSubRegValues |
| pruneSubRegValues, isDefInSubRange, pruneMainSegments | 3416, 3487, 3496 | D (C1) | subranges |
| removeImplicitDefs | 3510 | D (C3) | as eliminateUndefCopy |
| JoinVals::eraseInstrs | 3522 | T | |
| joinSubRegRanges, mergeSubRangeInto | 3616, 3678 | D (C1) | subranges |
| isHighCostLiveInterval | 3702 | T | |
| joinVirtRegs, joinIntervals | 3714, 3902 | T | |
| buildVRegToDbgValueMap, checkMergingChangesDbgValues(Impl) | 3908, 3952, 3970 | D | debug info not in LIR: no date |
| compareMBBPriority, isLocalCopy | 4058, 4080 | T | |
| lateLiveIntervalUpdate | 4096 | D | with debug info and ToBeUpdated: no date |
| copyCoalesceWorkList, copyCoalesceInMBB, coalesceLocals, joinAllIntervals | 4108, 4201, 4256, 4265 | T | |
| isTerminalReg, applyTerminalRule | 4142, 4152 | D (C7) | cannot fire (off at the pin) |
| run | 4326 | T | |

### DEVIATE exits

- C1 lanes (fires: 5316 sub-register copies in 88 of 130 m16 runs). Blocker: a sub-register operand model in LIR (AL/AH/AX of one value) and `LiveRange::SubRange` in `liveness/`; both belong to the LIR data-model work that waits for parity green. Proposed: LiveInterval subranges 11-12, the lane functions above 11-20 (p). Until then the cost is not measured; the first measurement is a count of byte copies the coalescer leaves, taken with the lane code in.
- C2 physical-register copies (fires: 3256 in 88 m16 runs). Ours are constraints on the instruction (`requires`/`delivers`), joined by the allocator's hints. Blocker: physical-register units in `LiveIntervals`. Proposed 11-14 (p); first the question of whether the constraint form loses a join the pin makes, measured on the copies the greedy leaves next to a fixed register.
- C3 implicit defs (8 in 4 m16 runs, 0 at m32): w-1's rows 59/61; translate 2 days after they land.
- C5: 11-06. C6: with C1. C7: no exit, cannot fire; reopened if the pin's default changes. Debug values: no exit, cannot fire; reopened if LIR gains DBG_VALUE.

## Row 66, RegAllocGreedy.cpp, in the pin's order

| Pin function | Line | Status | Blocker and date |
|---|---|---|---|
| LRE_CanEraseVirtReg, LRE_WillShrinkVirtReg, LRE_DidCloneVirtReg | 370, 385, 395 | T (`liverangeedit.rs`) | |
| enqueue, dequeue | 420, 517 | T | |
| tryAssign | 532 | T, zero register costs | CSR below |
| evictInterference, tryEvict | 617, 715 | T | |
| addSplitConstraints, addThroughConstraints | 739, 812 | T | |
| growRegion, calcCompactRegion | 872, 951 | T | |
| calcBlockSplitCost, calcGlobalSplitCost | 994, 1013 | T | |
| splitAroundRegion | 1067 | T | |
| tryRegionSplit, calculateRegionSplitCost(AroundReg), doRegionSplit | 1206, 1239, 1316, 1334 | T | |
| trySplitAroundHintReg | 1376 | - | 11-10 |
| tryBlockSplit, tryInstructionSplit | 1469, 1591 | T | |
| calcGapWeights, tryLocalSplit | 1666, 1744 | T | |
| trySplit | 1975 | T | |
| mayRecolorAllInterferences, tryLastChanceRecoloring, tryRecoloringCandidates | 2045, 2132, 2292 | T | |
| selectOrSplit | 2328 | T | |
| calcSpillCost | 2356 | - | CSR cost: 11-12 |
| tryAssignCSRFirstTime | 2384 | - | 11-12 |
| aboutToRemoveInterval | 2418 | - | with hint recoloring: 11-10 |
| initializeCSRCost | 2423 | - | 11-12 |
| collectHintInfo, getBrokenHintFreq, tryHintRecoloring, tryHintsRecoloring | 2471, 2516, 2534, 2643 | - | 11-10 |
| selectOrSplitImpl | 2655 | T | |
| report, computeStats, reportStats | 2757, 2786, 2878, 2904 | - | statistics only, no code effect: not planned |
| RAGreedy::run, runOnMachineFunction, getAnalysisUsage, releaseMemory, hasVirtRegAlloc | 2942, 289, 341, 413, 2930 | D | the pass runs inside RegAlloc: pass plumbing, not translated by design |

First translation PRs: C5 (11-06), then hint recoloring with trySplitAroundHintReg (11-10), CSR cost (11-12), each its own PR after the spiller steps land, tested on the rows pair.
