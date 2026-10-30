# TwoAddress: what is left (row 62) — owner w-1

Pin `TwoAddressInstructionPass.cpp` at d1106deb71cc. Translated: `isPlainlyKilled`/`isKilled` on one `LiveIntervals` (#4009), `isProfitableToCommute`/`commuteInstruction`/`tryInstructionCommute`, `isProfitableToConv3Addr`/`convertInstTo3Addr`, `rescheduleMIBelowKill`/`rescheduleKillAboveMI`/`isDefTooClose`, `noUseAfterLastDef`, `isRevCopyChain`, `isCopyToReg`. Not translated, in this order:

1. **2026-11-06 — the maps** (`SrcRegMap`/`DstRegMap`, `processCopy`, `scanUses`, `findOnlyInterestingUse`, `removeClobberedSrcRegMap`, `getMappedReg`): DEVIATE TA-1 narrows to the copies being the `delivers`/`requires` fields of the instruction carrying them. The AM-TA-9/10 rows are re-measured here.
2. One PR each, two days apart: `collectTiedOperands` 11-08; `processTiedPairs` (ours `_untied`: the undef-source copy, the memory destination through a register) 11-10; `tryInstructionTransform`'s order with the unfold of a folded load 11-12; `run`'s per-block `TiedOperandMap`/`DistanceMap`/`Processed` 11-14; `convertToThreeAddress` gaps (`isConvertibleLEA`, sizes) 11-17.
- **Not applicable**: `processStatepoint` (no statepoints), `eliminateRegSequence` (no REG_SEQUENCE at i486 and the 16-bit target).

Each step: objects on the 568 sweep, rows and QCport bytes, ledger d_faces Ir; a changed row is numbered.
