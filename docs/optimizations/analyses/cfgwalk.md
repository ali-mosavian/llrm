> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/cfgwalk.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/cfgwalk.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/cfgwalk.rs sha256:6ceca31bb6709f765be38af7de4a7f3439df2ad12d254b92bff771e0b2d18585 -->

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

DEVIATE: LLVM walks a block's predecessors in use-list order, newest use
first; MIR lists them in use order, oldest first, so the walk takes them
reversed.

## Used by
`adce` in llrm-transforms.
