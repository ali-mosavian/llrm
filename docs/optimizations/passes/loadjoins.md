> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loadjoins.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loadjoins.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loadjoins.rs sha256:4fe087fc1f11ba3cb7fd44d723f02de2e6a20f4ac044164313b958efc2043367 -->

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

```text
 b0: br c, b1, b2                  b0: br c, b1, b2
 b1: %y = load @g                  b1: %y = load @g
     br b3                             br b3
 b2: br d, b3, b4          =>      b2: br d, b5, b4
 b3: %x = load @g                  b5: %n = load @g ; the edge's own
     ret %x                            br b3
                                   b3: %x = phi [%y, b1], [%n, b5]
                                       ret %x
```

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
