> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopsimplifycfg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopsimplifycfg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopsimplifycfg.rs sha256:4fe06787800f68881cbb0bd8335e58dc04c0dfc9cdf178ca1b11e585b65d59a9 -->

# loopsimplifycfg

LoopSimplifyCFG: the branches and switches of a loop with a constant
condition become jumps, the blocks that leaves unreachable go, and a block
with one predecessor that jumps to it is merged into it.

## How it works
A translation of LLVM's
[`LoopSimplifyCFG.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopSimplifyCFG.cpp)
`simplifyLoopCFG` (:693) at d1106deb71cc. MIRROR: `constantFoldTerminators`
works out, from the blocks in reverse postorder, which blocks and exits of
the loop stay reachable once every constant terminator is a jump
(`ConstantTerminatorFoldingImpl::analyze`); it gives up on an irreducible
loop, one entered indirectly, one with nothing to fold, one whose latch
would go, one with blocks that would stay but leave the loop, and one with
a dead exit and an LCSSA form broken by a token. Else the dead exits
keep their static reachability from a switch on `0` in the preheader
(`handleDeadExits`), the terminators are folded (`foldTerminators`), and
the dead blocks are emptied and deleted (`deleteDeadLoopBlocks`). Then
`mergeBlocksIntoPredecessors` merges the loop's blocks.

DEVIATE 1: the slot runs the pass over every loop, innermost first, before
the next slot; the pin's loop pass manager runs LoopInstSimplify,
LoopSimplifyCFG, LICM and the rest on one loop, then the next.

DEVIATE 2: no DomTreeUpdater, loop info or MemorySSA to update: the tree
and loops are made again after a loop changed. The LCSSA of the loops a
loop leaves (`formLCSSARecursively` on the outermost it left) is formed over
the function, after the edit and not before the folding.

DEVIATE 3: no branch weights on the dummy switch (MIR has none).

## Parity slot
`loop-simplifycfg@716`, after LoopInstSimplify in `LPM1`
([PassBuilderPipelines.cpp#L716](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L716)).

## Example
A loop whose body tests a constant `false` and leaves for an exit on true
loses the test: the exit's edge from the loop goes, the exit stays
reachable from a `switch i32 0` in the preheader until SimplifyCFG.

## Used by
Named by `pipeline`.
