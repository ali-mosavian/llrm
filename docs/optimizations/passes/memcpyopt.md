> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/memcpyopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/memcpyopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/memcpyopt.rs sha256:c639c4af202bd3b61a6a36b0310660befdd12b92c1ed20a6d75ccaf3645a7fa4 -->

# memcpyopt

MemCpyOpt on MemorySSA: LLVM's `MemCpyOptPass` apart from the memset
merging of `processStore` and `processMemSet`, which
[`crate::memcpyoptimizer`](memcpyoptimizer.md) does (stage 1). A translation of
`MemCpyOptimizer.cpp` at d1106deb71cc.

## How it works
Each memcpy, memmove and aggregate store of a load is looked at in block
order with the memory graph (`mssa`) kept right by the updater as
instructions are made and erased. The clobber a memcpy's source or
destination walks up to decides what it can become:
- a copy to itself, or of no bytes, is gone; a copy out of a constant global
  that repeats one byte is a memset;
- a memcpy of a memcpy's destination copies from that memcpy's source
  instead (`processMemCpyMemCpyDependence`);
- a memcpy right after a memset of its destination: the memset shrinks to
  what the copy leaves (`processMemSetMemCpyDependence`);
- a memcpy of a memset's bytes is a memset (`performMemCpyToMemSetOptzn`);
- a copy of memory nothing wrote (a fresh alloca) is gone
  (`hasUndefContents`);
- a call that fills a fresh alloca, copied to another place, fills that
  place instead and the copy is gone (`performCallSlotOptzn`, from a memcpy
  or a load and store of the alloca);
- a copy of a whole stack slot to another that never conflict with it makes
  the two one slot (`performStackMoveOptzn`);
- a `byval` argument, or one a callee only reads, fed by a memcpy takes the
  memcpy's source (`processByValArgument`, `processImmutArgument`);
- a memmove whose ranges cannot meet is a memcpy (`processMemMove`);
- an aggregate loaded and stored right after is a memcpy, or a memmove where
  the store may write what the load reads (`processStoreOfLoad`), the store
  lifted above the first write to the loaded bytes when nothing in between
  needs it (`moveUp`).

## Example
A copy of a copy reads the first source:

```text
 call memcpy(%b, %a, 16)           call memcpy(%b, %a, 16)
 call memcpy(%c, %b, 16)    ==>    call memcpy(%c, %a, 16)
```

## DEVIATEs
1. Alias questions are `mssa::alias`'s (llrm's reference model), not
   BasicAA's, and there is no `EarliestEscapeAnalysis`: what it adds to
   `BatchAAResults` is a capture query the reference model answers by the
   object's exposure. Exit: none; the model is llrm's.
2. Nothing throws (no unwinding in MIR): `mayBeVisibleThroughUnwinding` is
   false.
3. No debug-info assignment IDs (`mergeDIAssignID`), no `combineAAMetadata`
   (the formed calls carry no `!tbaa`, as in `loopidiom`).
5. MIR holds no constant expression in code: where the builder folds
   `getelementptr (i8, @g, 16)` the pass makes the instruction. A forced
   inline copy (`llvm.memcpy.inline`) is not forwarded, as the builder's
   `CreateMemCpyInline` has no counterpart here.
9. `getOrEnforceKnownAlignment` is `known_alignment`: it never raises an
   alloca's or global's alignment, so a source aligned less than the byval
   or the alloca needs ends the forwarding. Exit: the alignment-raising
   helper (needs the writer of alloca alignment shared with call slot).
6. `moveUp` does not lift a call (LLVM does when it neither reads nor writes
   what the lifted ones touch): a call between the load and the store ends
   the attempt. Fact: `getModRefInfo(Instruction, CallBase)` has no model
   here. Exit: a call-against-call mod/ref query.
7. The call slot asks `PointerMayBeCaptured` of the destination object where
   the pin asks `PointerMayBeCapturedBefore` the call (only when the callee
   may capture the source): a capture after the call also stops it. Exit: a
   dominance-aware capture query.
8. The call slot gives up where the alias model says the call may touch the
   destination; the pin goes on to `callCapturesBefore`. Exit: that query in
   `llrm_analysis::aa`'s model.

## Runs
In the slot of `memcpyopt@818`.

## Keeps
The blocks and edges: only instructions change.

## See also
[`crate::memcpyoptimizer`](memcpyoptimizer.md), [`crate::deadstore`](deadstore.md), the memory graph in
`llrm_analysis::mssa`.
