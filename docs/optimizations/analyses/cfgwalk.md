> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/cfgwalk.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/cfgwalk.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/cfgwalk.rs sha256:152f0797f66955f11845084d242aaeacc8b99d0d917dce091bd95f717fd27c5f -->

# cfgwalk

Walks of a function's CFG that LLVM's passes ask for by name.

## How it works
`find_function_backedges` is `FindFunctionBackedges` (`Analysis/CFG.cpp`): a
depth-first search from the entry with three states per block; an edge to a
block still on the stack is a back edge. It is cheaper than dominators and
loop info, and differs from them on irreducible loops.
`inverse_post_order_ext` is `inverse_post_order_ext(BB, Visited)` of
`ADT/PostOrderIterator.h`: the post-order of the walk over predecessors from
a block, skipping blocks the caller's set has seen, which it fills. Both are
linear.

## Used by
`adce` in llrm-transforms.

## Example
In a loop the edge to a block still on the stack is the back edge:

```text
entry -> head -> body -> head   ; body -> head is the back edge
```
