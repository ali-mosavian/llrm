> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/local.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/local.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/local.rs sha256:aada91e94acf06850fc46b5b235cf2d80fad9b2d763a73cf06b68da02218d535 -->

# local

The utilities GVN calls on a CSE: patching the surviving instruction,
replacing the uses a dominator tree allows, dead and duplicate-phi
cleanup, block merging, and the pointer-equality replacement checks.

A translation of LLVM's
[`Local.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp)
and the parts of `Dominators.cpp`, `BasicBlockUtils.cpp`, `Loads.cpp`,
`Instruction.cpp` and `Attributes.cpp` it calls, at d1106deb71cc. Each
function names the C++ function it translates, keeps its order of checks,
and uses LLVM's name in snake case. Not called yet.

## How it works
MIR has no `Use` object: a use is `Use` (user and operand index), and a
phi's value operand `i` comes in on the block operand `i + 1`. `RAUW` is
`Function::replace_value`, which also points a debug record at the new
value, as LLVM's metadata-as-value does.

DEVIATE, each with the fact MIR lacks:
1. `MDNode::getMostGeneric*`, `intersect`, and the merged `!prof`,
   `!callees`, `!captures`: MIR's nodes are untyped tuples and nothing
   builds a merged one. A kind both instructions carry in two different
   nodes is dropped, which LLVM may keep merged. Two non-distinct nodes with
   equal operands are one node, as LLVM uniques them.
2. `!DIAssignID`, `!mmra`, `!memprof`, `!callsite`, `!callee_type`,
   `!alloc_token`: nothing in MIR reads them (debug info is
   `Function::debug_records`, not metadata). They are dropped, the arm for a
   kind LLVM does not know.
3. `ConstantRange::unionWith` for a `range` attribute: MIR has no
   `ConstantRange`. Two different ranges drop the attribute.
4. `addrspacecast nonnull`: `Flags` has no such bit.
5. `dropPoisonGeneratingFlags` on `ctlz`, `cttz` and `abs`: MIR has none of
   them.
6. `llvm.fake.use`: not in the intrinsic table, so no use is skipped.
7. `DbgLabelInst`: a debug label is not an instruction here.
8. `isRemovableAlloc`, `getFreedOperand`, `isMathLibCallNoop`: MIR has no
   `allockind` attribute and no `TargetLibraryInfo`, so no call is a known
   allocation, free or libm function.
9. `experimental_guard`, `wasm_trunc_*`, `ptrauth_*`, `stacksave`,
   `launder_invariant_group`, `allow_runtime_check`, `allow_ubsan_check` and
   the constrained FP intrinsics: not in the intrinsic table.
10. A non-volatile load from a constant global: a load has no atomic
    ordering, so every one without side effects is already dead above it.
11. A lifetime marker's pointer is operand 1 (`size, ptr`). A constant
    pointer's uses are not tracked per function, so its markers are kept.
12. `isAssumeWithEmptyBundle`: MIR has no operand bundles, so it is true.
13. `mayThrow` of an invoke needs `canUnwindPastLandingPad`: an invoke may
    throw.
14. `isIdenticalTo` and `isIdenticalToWhenDefined`: phis only, the one
    caller. A phi's blocks are operands, so comparing them compares blocks.
15. `MergeBlockIntoPredecessor`'s `DomTreeUpdater` and `DominatorTree`,
    `LoopInfo`, `MemorySSAUpdater` and `MemoryDependenceResults`: MIR passes
    hold none of them, the caller rebuilds. The debug-only `IncomingValues`
    goes with them.
16. `MergeBlockIntoPredecessor`'s `hasAddressTaken` (MIR has no
    `blockaddress`), `HasLoopOrEntryConvergenceToken` (no convergence
    tokens), `PTI->mayHaveSideEffects` (after `isSpecialTerminator` a
    terminator is a `ret`, `br`, `switch` or `unreachable`, which never
    writes, throws or fails to return), `takeName` (a `Function` has no
    block-name setter), and the `unreachable` left in the merged block (the
    block is erased at once).
17. `PtrToAddrInst`: MIR has no `ptrtoaddr`. `getUnderlyingObjectAggressive`
    is `valuetracking::underlying`, which does not look through a `select`,
    a `phi` or a call that returns an argument: two pointers that differ
    only below one are not replaceable here, where LLVM may say they are.
24. `MergeBlockIntoPredecessor` erases `PTI` after redirecting the uses of
    `BB`; here before. A `DbgMarker` hands `PTI`'s records to the next
    instruction or the trailing list, which `STI` adopts when it moves.
    `Function::erase` re-homes a record when its instruction goes
    (`settle_records`), and the branch must still name `BB` for that to
    reach `STI`: redirected first, it names its own block and the records
    stay on the erased branch. Asserted by
    `merging_leaves_the_records_of_the_old_branch_before_the_new_terminator`.
25. `!llrm.descriptor` (ours, no LLVM kind): kept where both instructions
    carry it, dropped otherwise. LLVM's unknown-kind arm would drop it, and
    the merged `inttoptr` then reads as a forged selector (`forging`).

## Example
Two phis with the same arms in one block. Before:

```mir
%p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
%q = phi i16 [ %x, %b1 ], [ %y, %b2 ]
```

`eliminate_duplicate_phi_nodes` puts `%q` in the set to remove and makes it
read `%p`; after the caller erases it only `%p` is left.

From unit test `a_duplicate_phi_is_replaced_by_the_first` in
`local_tests.rs`.

## Used by
Nothing yet; `gvn` is the caller this is for.
