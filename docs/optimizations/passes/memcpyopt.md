> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/memcpyopt.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/memcpyopt.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/memcpyopt.rs sha256:d43c4fef13bcf56cbf078c52a59668f99e38420deab3a7a9fbf5f729eea0258a -->

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
- a memmove whose ranges cannot meet is a memcpy (`processMemMove`).

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
4. Done later (stages): `processStoreOfLoad` and `moveUp`,
   `performCallSlotOptzn`, `performStackMoveOptzn`, `processByValArgument`
   and `processImmutArgument`.
5. MIR holds no constant expression in code: where the builder folds
   `getelementptr (i8, @g, 16)` the pass makes the instruction. A forced
   inline copy (`llvm.memcpy.inline`) is not forwarded, as the builder's
   `CreateMemCpyInline` has no counterpart here.

## Runs
In the slot of `memcpyopt@818`.

## Keeps
The blocks and edges: only instructions change.

## See also
[`crate::memcpyoptimizer`](memcpyoptimizer.md), [`crate::deadstore`](deadstore.md), the memory graph in
`llrm_analysis::mssa`.
