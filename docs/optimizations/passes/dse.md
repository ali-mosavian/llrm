> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dse.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dse.rs sha256:98c4e1425d5585ca6e0984d778c926fe521742a9ed3df2bdf81987d209aacf46 -->

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

## Example
Before:
```mir
define i16 @f(i16 %x) {
b0:
  %s = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %s, i8 0, i16 8, i1 false)
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %w = sext i16 %i to i32
  store i32 %w, ptr %s
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, %x
  br i1 %go, label %b1, label %b2

b2:
  ret i16 %x
}
```
Nothing reads the slot, so the memset and the loop's store are dead; the
test runs `dse` then `dead` and asserts that no `alloca`, `store` or `call
void` is left.

From unit test `test_a_zeroed_slot_nothing_reads_goes` in `dse_tests.rs`.

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
[`crate::dead`](dead.md), [`crate::promote`](promote.md), `crate::loopmotion`.
