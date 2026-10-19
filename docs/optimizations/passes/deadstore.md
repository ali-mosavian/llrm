> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/deadstore.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/deadstore.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/deadstore.rs sha256:0776389167e1d9919581a4316a81e48405e237fe5daf3d197e5b5bea1b101fd7 -->

# deadstore

Dead store elimination: LLVM's `DSEPass`, stage 1 (`DSEState`,
`eliminateDeadDefs`, `getDomMemoryDef`,
`eliminateDeadWritesAtEndOfFunction`, `storeIsNoop`).

## How it works
MemorySSA is built once. Every store and every `llvm.memset` of a constant
length is a candidate killing def. From a killing def the walk goes up its
defining accesses (`getDomMemoryDef`) to the nearest earlier def that it
completely overwrites, with nothing between that reads the bytes (a use of
the dead def that may read them ends the walk); a dead def reached through a
`MemoryPhi` is searched from each incoming access. The dead def is removed
(`deleteDeadInstruction`: its access leaves MemorySSA, its operands that
became trivially dead go with it). A def to a location that the caller
cannot see after the function returns (a local object) is killed also by
the function's exits: no path from it to a return reads it
(`eliminateDeadWritesAtEndOfFunction`, `isWriteAtEndOfFunction`), or every
path to an exit passes a block that overwrites it (the common
post-dominator check at the end of `getDomMemoryDef`). A store that writes
back the value it just loaded from the same place, or the byte the object
already holds, is a no-op (`storeIsNoop`).

```text
  store i16 1, ptr %p        (overwritten before any read)
  store i16 2, ptr %p   ==>  store i16 2, ptr %p
  %v = load i16, ptr %q
  ret i16 %v                 (a store to a local nothing reads is gone too)
```

MIRROR: `eliminateDeadStores`, `DSEState::eliminateDeadDefs` (both),
`getDomMemoryDef`, `isOverwrite` (without the masked and scalable cases),
`isWriteAtEndOfFunction`, `eliminateDeadWritesAtEndOfFunction`,
`isReadClobber`, `isCompleteOverwrite`, `isGuaranteedLoopIndependent`,
`isGuaranteedLoopInvariant`, `canSkipDef`, `isRemovable`, `isDSEBarrier`,
`deleteDeadInstruction`, `storeIsNoop`, `pushMemUses`.

DEVIATE 1: alias questions are `mssa::alias` (llrm's far-pointer
semantics), not BasicAA's address-space view; the underlying object of a
location is the one object of its provenance, and "must alias with an
offset" is the same address with sizes (`AliasResult::Partial`). Exit:
none; the model is llrm's.
DEVIATE 2: nothing throws (no unwinding in MIR), so `ThrowingBlocks` is
empty and every local object is invisible on unwind. Exit: when MIR has EH.
DEVIATE 3: an object is invisible after the function returns when it is
a stack or frame object (`isa<AllocaInst>`); `dead_on_return` and byval
arguments (stage 3), and non-alloca objects with no capture (malloc) are
absent. Exit: stage 3.
DEVIATE 4: memory terminators (`lifetime.end`, `free`), the `initializes`
attribute, `tryFoldIntoCalloc` and
`eliminateRedundantStoresViaDominatingConditions` are stage 2 and 3. Partial
overwrites (`isPartialOverwrite`, `IOLs`,
`tryToMergePartialOverlappingStores`, `tryToShorten` of memsets) and
`eliminateRedundantStoresOfExistingValues` are in. Exit: stage 2.
DEVIATE 5: only stores and constant-length memsets are candidates, and
`storeIsNoop` knows no `getInitialValueOfAllocation` (it needs `calloc`
and friends). Exit: stage 2.

## Example
A local written and never read:

```mir
%a = alloca i16
store i16 7, ptr %a     ; gone: nothing reads %a before the return
ret i16 0
```

## Runs
In the slot of `dse@820` (LLVM's order); the older `dse.rs` runs elsewhere.

## Needs
MemorySSA (`mssa`), its alias query, dominators and post-dominators, the
loops.

## Keeps
The blocks and edges: only instructions are removed.

## See also
LLVM
[`DeadStoreElimination.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DeadStoreElimination.cpp).
