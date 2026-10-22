# TwoAddress: rescheduleMIBelowKill and rescheduleKillAboveMI

Row 62 rest. Pin d1106deb71cc, [TwoAddressInstructionPass.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp): `rescheduleMIBelowKill` ([L928](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L928)), `isDefTooClose` (L1100), `rescheduleKillAboveMI` ([L1116](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1116)); called from `tryInstructionTransform` ([L1385](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1385), [L1418](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TwoAddressInstructionPass.cpp#L1418)). Landed before this: commute (#3637), convertInstTo3Addr at m32 (#3680) and m16 (#3729), the kills without LiveIntervals (TA-6, #3803).

## What they do

When a tied instruction's source lives on only for one later local reader, move the instruction below that reader (`MIBelowKill`) or the reader above the instruction (`KillAboveMI`), so the source dies at the tied instruction and no copy is made. Both stop at calls, branches, terminators, side effects and copies, look at most 10 instructions, never move a load past a store, and keep other live ranges as they are.

## Ours

`tied()` walks a block once and builds the output list. The pieces need: a working list the walk can reorder, the kill table kept up when an instruction moves (the pin's `LV->removeVirtualRegisterKilled`/`addVirtualRegisterKilled`), `isSafeToMove` (no store, no volatile, no call; loads do not cross a store), and the pin's latency test (`getInstrLatency > 1`): without itineraries `TargetInstrInfo::getInstrLatency` is 2 for an instruction that may load and 1 otherwise ([TargetInstrInfo.cpp#L1832](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetInstrInfo.cpp#L1832)), so the test refuses a load; translated as `may_load` (DEVIATE TA-7: no scheduling model, exit: one). Whether x86 hands the pass itineraries at all is not checked.

## Order

1. `isSafeToMove`, `regOverlapsSet` and the working list; `rescheduleMIBelowKill` with the 10-instruction window. Fail-first: a body where the source's only other reader is one instruction away: no copy is made.
2. `isDefTooClose` and `rescheduleKillAboveMI`.
3. Each priced on rows, QCport and compile Ir; wrong-output guards as always.

Dates: 1 by 2026-11-02, 2 by 2026-11-04. Owner w-1.
