> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dse.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dse.rs sha256:5b1a311ce3d648d97aca7cc4789308cc3af73255f706b41987044469a825c36b -->

# dse

Dead store elimination: LLVM's DSE. A store is removed when nothing can read
the bytes it wrote.

## How it works
A store is dead when every path overwrites its cell before reading it, or
when nothing after it can read the cell. [`llrm_analysis::avail`](../analyses/avail.md)'s
`dead_stores_escaping` finds them over the function's memory accesses
(`memoryssa::Accesses`): a backward dataflow over cells of what is stored
again before any read. The cells no call and no exit can observe, which
`observers::private_of` computes from the `Pointers` and `Published`
analyses, are unread once the function returns, so a store to a local that
nothing reads afterwards is dead. An access through a pointer that no fact
follows spares only a frame cell whose object had not escaped by then
(`Pointers::escaped_before`).

```text
 store i16 1, ptr %p          (overwritten before any read)
 store i16 2, ptr %p    ==>   store i16 2, ptr %p
 ret                          ret
```

A call's reach is its `Summaries` footprint (reads, writes, `initializes`
bytes) where the pass manager requires them; a bare manager takes every
callee as unknown. A call that fills a buffer kills the stores before it
that it overwrites; a `llvm.memset` whose bytes later stores together
overwrite goes.

`promote` leaves every store when it replaces loads by values; this is the
pass that removes those that are now read by nothing.

## Runs
Once per pass of the function fixed point, after `trivialunswitch` and
`inferspace` and before `gvn`. Off with `-fno-tree-dse`; nothing runs at
-O0.

## Needs
`Accesses`, `Pointers`, `Published` and the CFG `Shape`; reads the memory
operations and the CFG.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## See also
[`crate::dead`](dead.md), [`crate::promote`](promote.md), [`crate::loopmotion`](loopmotion.md).
