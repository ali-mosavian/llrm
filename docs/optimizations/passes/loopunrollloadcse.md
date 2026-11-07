> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopunrollloadcse.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopunrollloadcse.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopunrollloadcse.rs sha256:68815fd1b4cf26cb10adf3da383366c9d2cec7c36cc8a6386ffaa37f6706efff -->

# loopunrollloadcse

`loadCSE`
([LoopUnroll.cpp:282](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUnroll.cpp#L282))
and `getMatchingValue` (L260): after a partial unroll, a simple load whose
address has the SCEV of an earlier load that dominates it and that nothing
may have written since is that load's value.

The dominator tree of the loop is walked from the header with a scope per
node, the loads available at it keyed by the SCEV of their address. Each
entry carries the generation of memory when it was made: a block with
more than one predecessor and every instruction that may write memory
start a new one. A load of the same generation is the available value; one
of an older generation is it where MemorySSA's clobbering access of the
load dominates the access of the available one.

## Deviations
- LC1 MemorySSA is made over a copy of the whole function (LLVM: over the
  loop, `MemorySSA(*L, AA, DT)`), once, and only when two loads of the loop
  share an address SCEV: without such a pair nothing can match. It is not
  updated as loads go, as LLVM's is not. Exit: a loop-scoped build.
- LC2 The SCEVs are made before the walk, by the pass's `ScalarEvolution`
  (the one `simplifyLoopIVs` asked), since the walk changes the function and
  LLVM's holds its expressions current.

## Used by
[`crate::loopunrollpass::unroll_loops`](loopunrollpass.md), where the loop is still a loop.
