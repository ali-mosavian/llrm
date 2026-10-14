> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loadjoins.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loadjoins.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loadjoins.rs sha256:5c5f11c870fbe80638db2030a8753f089c4982c5a8fafbf25e0c896d78a8a768 -->

# loadjoins

Load PRE at joins: a load whose bytes every predecessor already holds
becomes a phi of those values, and a predecessor holding none may get a
load of its own on its edge.

## How it works
For each block with two or more predecessors, each load in it is a
candidate. For every incoming edge the pass looks for a provider: a store
(`avail::stored_from`) or a load (`avail::loaded_into`) of the same bytes
(`memoryssa::same_bytes`), of the load's type, holding a value or a
constant. A provider is accepted when
- it dominates the predecessor but not the join (one that dominates the join
  is `gvn`'s plain redundancy);
- no loop contains it without containing the join;
- nothing on the way from it to the edge may write the bytes (`MemoryDef`s
  of `memoryssa`, `available_on_edge`), calls judged by their `CallEffects`.

Of several providers the deepest in the dominator tree wins, the latest in
its block. When the load's pointer is a phi of the join, the bytes are
translated to that phi's input on the edge before the search, and an
address so translated carries alias's provenance.

The load is replaced by a phi of the providers' values when every edge has
one. With `insert`, an edge without a provider may instead get a copy of
the load, if at least one edge holds a value (`insertable`):
- the edge's source ends in a `br`; it is the only exit of the predecessor
  or one side of a conditional branch, which is then split (`edges::split`)
  so the other side is not touched;
- the edge is no back edge and crosses no loop boundary;
- everything before the load in the join is a value that may be computed
  anywhere (`llrm_mir::memory::only_value`) and no load;
- the pointer is defined above the edge.

## Example
`@g + 32` is a cell stored in `b0`. The load in `b3` is reached along `b1`,
which loads it, and along the edge from `b2`, which does not; with `insert`
that edge gets a load of its own. Before:

```mir
define i16 @f(i1 %c, i1 %d, i16 %v, i16 %s) {
b0:
  store i16 %v, ptr getelementptr (i8, ptr @g, i16 32)
  br i1 %c, label %b1, label %b2

b1:
  %y = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3

b2:
  br i1 %d, label %b3, label %b4

b3:
  %x = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  ret i16 %x

b4:
  ret i16 0
}
```

After:

```mir
b2:
  br i1 %d, label %0, label %b4

b3:
  %x1 = phi i16 [ %y, %b1 ], [ %1, %0 ]
  ret i16 %x1

0:
  %1 = load i16, ptr getelementptr (i8, ptr @g, i16 32)
  br label %b3
; ... b0, b1 and b4 unchanged
```

`b2` also leads to `b4`, so its edge to `b3` is split and the new block `0`
loads; `b4` reads nothing new.

From unit test `test_missing_conditional_edge_gets_its_own_load_block` in
`loadjoins_tests.rs`.

## Runs
Not a pass of the pipeline: [`crate::gvn`](gvn.md) calls `reused` after its
value numbering and join PRE, with `insert` set when `pre` is on (gcc's
`-ftree-pre`, -O2 and up) and the round made no join PRE. `LoadJoins` is
the same step as a function pass, for tests.

## Needs
`memoryssa::Accesses` and the managed `Pointers` (a missing `Summaries`
means every call is unknown, so less is found), `cfg::Shape`.

## Keeps
Nothing when it changes a body: a split edge changes the CFG.

## See also
LLVM's GVN `performLoadPRE`; gcc's PRE of loads, `-ftree-pre`.
[`crate::gvn`](gvn.md) describes the surrounding pass.
