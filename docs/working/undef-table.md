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
| InstSimplify | `simplifyPHINode`, `simplifySelectInst`, `simplifyICmpInst`, `simplifyDivRem`, `simplifyAdd/Sub/Mul/And/Or/Xor/Shl/RightShift`, `isPoisonShift`, `expandBinOp` (`getWithoutUndef`), `threadBinOpOverSelect`, `simplifyGEPInst` | DONE |
| InstSimplify (rest) | `simplifyInsertValueInst` (#2250), `simplifyWithOpsReplaced` (`CanUseUndef`), `simplifyAndOrWithICmpEq`, the intrinsics (#2248) | OWED |
| ValueTracking | `isGuaranteedNotToBeUndefOrPoison` L8154, `computeKnownBits` L2647, phi `hasConstantValue` L2004, `isBytewiseValue` L6744 | DONE |
| LVI, CVP | `ValueLatticeElement::get` (undef state), `getValueFromICmpCondition` L1390 | DONE |
| GVN | `analyzeLoadAvailability` L1443 (load of an alloca is undef) | OWED with load PRE (not ported); dead-block poison is the pin's poison: DONE |
| EarlyCSE, PHITransAddr | vector masks only | N/A |
| SimplifyCFG (Local.cpp) | `CanMergeValues`, `selectIncomingValueForBlock`, `gatherIncomingValuesToPhi`, `replaceUndefValuesInPhi` | DONE |
| SimplifyCFG (rest) | `markAliveBlocks` store to undef ptr (Local L2758), `assume(undef)` L2703, lookup tables L7198, `passingUndefUB` L9174 | OWED |
| JumpThreading | `getKnownConstant` L543, branch on undef L1009, xor L1838 | OWED |
| SCEV | the pin has no `UndefValue` arm; an undef is an unknown | N/A |
| LoopIdiom | `isa<UndefValue>` of the splat and pattern value L736/L741, the memset byte | DONE |
| backend globals | an undef initializer is zero bytes | DONE |
| InstCombine, IndVars `hasConcreteDef`, FunctionAttrs return-null, MemCpyOpt `isZeroSize`, GlobalOpt undef initializers, SCCP | the passes or functions are not ported | N/A until ported |
| LICM, LSR | no undef arm that MIR has | N/A |
| isel, legalize | undef lowers as any value: zero; `freeze undef` selected as a constant | DONE (#2532, guard test) |
