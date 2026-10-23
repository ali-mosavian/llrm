# TwoAddress: what is left (row 62) — owner w-1

Pin `TwoAddressInstructionPass.cpp` at d1106deb71cc. Translated: `isPlainlyKilled`/`isKilled` on one `LiveIntervals` (#4009), `isProfitableToCommute`/`commuteInstruction`/`tryInstructionCommute`, `isProfitableToConv3Addr`/`convertInstTo3Addr`, `rescheduleMIBelowKill`/`rescheduleKillAboveMI`/`isDefTooClose`, `noUseAfterLastDef`, `isRevCopyChain`, `isCopyToReg`. Not translated, in this order:

1. **2026-11-27 — the maps**: `scanUses`, `processCopy`, `removeMapRegEntry`, `removeClobberedSrcRegMap`, `getMappedReg`, `regsAreCompatible`, `findOnlyInterestingUse`, `isTwoAddrUse` (DEVIATE TA-1: our `SrcRegMap`/`DstRegMap` are built from `requires`/`delivers`). The AM-TA-9/10 rows are re-measured here: the commute reads the maps as the pin does.
2. **2026-12-04 — the driver and the pairs**: `run`'s loop (`TiedOperandMap`, `DistanceMap` per block), `tryInstructionTransform` (commute, reschedule below, 3-address, reschedule above, unfold), `collectTiedOperands`, `processTiedPairs` (ours: `_untied`; the undef-source copy and the memory destination through a register are the cases it has).
3. **Not applicable**: `processStatepoint` (no statepoints), `eliminateRegSequence` (no REG_SEQUENCE at i486 and the 16-bit target).

Each step: objects on the 568 sweep, rows and QCport bytes, ledger d_faces Ir; a changed row is numbered.
