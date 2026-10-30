> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/mem2reg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/mem2reg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/mem2reg.rs sha256:0d613e981fc535ba2e8fb29f3e151bb8e4fbaa9780ed4b025321cc8a74f42da5 -->

# mem2reg

`mem2reg`: an alloca read and written only by direct loads and stores
becomes SSA values and phis, and its loads, stores and the alloca go. A
translation of LLVM's `PromoteMemToReg` and the pass around it
(`PromoteMemoryToRegister.cpp`, `Mem2Reg.cpp` at d1106deb71cc).

## How it works
Unlike [`crate::promote`](promote.md), which keeps every store for `dse`, this deletes
what it promotes. The steps of `PromoteMem2Reg::run` are in LLVM's order:
dead allocas, a single store, a single block, then phis at the iterated
dominance frontier of the stores where the alloca is live on entry, and a
renaming down the CFG from the entry.

LLVM structure (PromoteMemoryToRegister.cpp): `isAllocaPromotable` L66,
`AllocaInfo::AnalyzeAlloca` L235, `LargeBlockInfo` L330,
`removeIntrinsicUsers` L522, `rewriteSingleStoreAlloca` L593,
`promoteSingleBlockAlloca` L706, `PromoteMem2Reg::run` L800,
`ComputeLiveInBlocks` L1044, `QueuePhiNode` L1113, `RenamePass` L1147,
`PromoteMemToReg` L1264; `Mem2Reg.cpp` L34.

DEVIATE, each with its reason:
- Debug records: a `Declare` of the alloca becomes `Value` records of what
  is stored (`ConvertDebugDeclareToDebugValue`), a `Piece` where the stored
  value is narrower than the alloca. No assignment tracking (`dbg.assign`):
  MIR has none.
- No `llvm.assume`, so no `convertMetadataToAssumes` (`noundef` metadata)
  and no droppable users; no `fake_use`. Lifetime markers are translated.
- `isGuaranteedNotToBePoison` is true of a constant that is not `poison`
  only (no `noundef` arguments).
- The iterated dominance frontier is `Dominance::frontiers` iterated with
  the live-in pruning, which yields the set `ForwardIDFCalculator` does.
- `simplifyInstruction(PN)` is the phi case of `simplifyPHINode`, local
  until `simplify_phi` of `llrm_mir::simplify` lands.
- `NoSignedZeros` (fast-math flags of the phi): MIR phis carry none.

## Example
Before (from `test_stores_in_two_arms_make_a_phi_at_the_join`):
```mir
define i32 @f(i1 %c) {
b0:
  %s = alloca i32
  br i1 %c, label %b1, label %b2

b1:
  store i32 1, ptr %s
  br label %b3

b2:
  store i32 2, ptr %s
  br label %b3

b3:
  %x = load i32, ptr %s
  ret i32 %x
}
```

After:
```mir
define i32 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %x = phi i32 [ 1, %b1 ], [ 2, %b2 ]
  ret i32 %x
}
```

## Used by
[`crate::pipeline`](pipeline.md): the round's `Promote` is swapped for `Mem2Reg` when
`Options::mem2reg` is set (on by default; `LLRM_NO_MEM2REG` turns it off).
