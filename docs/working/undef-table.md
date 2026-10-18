# MIR `undef`: pass, pin function, state

`UndefValue` is a MIR constant (#2532). Each row is a place the pin
([d1106deb71cc](https://github.com/llvm/llvm-project/tree/d1106deb71cc81016406a1917d0508d2df296266))
treats undef apart from poison. State: DONE (translated), OWED (poison or
nothing today; an undef constant is an opaque value there, which is safe),
N/A.

| Pass | Pin function | State |
|---|---|---|
| mem2reg, SROA, Promote | `PromoteMemoryToRegister` L914, `SROA` L6393 | DONE (#2536) |
| ConstantFold (creation) | [`ConstantFoldBinaryInstruction`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ConstantFold.cpp#L637), `ConstantFoldCastInstruction`, `ConstantFoldCompareInstruction`, `ConstantFoldSelectInstruction` | DONE |
| ConstantFolding | `isa<UndefValue>` sites that read `is_poison` | OWED (vt-constantfolding) |
| InstSimplify | `simplifyPHINode`, `simplifySelectInst`, `simplifyICmpInst`, `simplifyDivRem`, `simplifyAdd/Sub/Mul/And/Or/Xor/Shl/RightShift`, `simplifyFreezeInst`, `simplifyGEPInst`, `simplifyInsertValueInst`, `simplifyWithOpsReplaced` | OWED |
| ValueTracking | `isGuaranteedNotToBeUndefOrPoison`, `computeKnownBits` of an undef | OWED (check) |
| LVI, CVP | `getFromConstant` (undef state), `ValueLatticeElement` | OWED: lattice folds poison into `Undef` |
| GVN, PHITransAddr, EarlyCSE | undef operands in value numbering | OWED |
| SimplifyCFG, JumpThreading | branch or switch on undef | OWED |
| SCEV | `getUnknown` of an undef | OWED (check) |
| LICM, LSR, indvars | none known | N/A until found |
| isel, legalize | undef lowers as any value: zero | DONE (#2532) |
