> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopinstsimplify.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopinstsimplify.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopinstsimplify.rs sha256:5bff7aa57078232b9a936f29437a9f6c247e70a97e8962411e1323dcecef70cf -->

# loopinstsimplify

LoopInstSimplify: the instructions of a loop that equal an operand or a
constant are replaced by it, and what that leaves dead goes.

## How it works
A translation of LLVM's
[`LoopInstSimplify.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopInstSimplify.cpp)
`simplifyLoopInst` (:41) at d1106deb71cc. MIRROR: the blocks of a loop are
walked in reverse postorder from its header, so a definition is met before
its uses but for phis; the first sweep tries every instruction, the next
ones only the users of what was replaced, and a phi already visited this
sweep that reads a replaced value sends the loop round again. A
replacement that would break LCSSA form is skipped. What nothing reads
goes when the sweep ends, with what only it read.

DEVIATE 1: the slot runs the pass over every loop, innermost first, before
the next slot; the pin's loop pass manager runs LoopInstSimplify,
LoopSimplifyCFG, LICM and the rest on one loop, then on the next.

DEVIATE 2: no MemorySSA to update (llrm builds it where a pass asks).

## Parity slot
`loop-instsimplify@715`, first of the loop pass manager `LPM1`
([PassBuilderPipelines.cpp#L715](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L715)),
after Reassociate and ConstraintElimination, at -O1 too.

## Keeps
Blocks and edges are unchanged: dominators and loops are preserved.

## Example
An `add %x, 0` in a loop body is replaced by `%x`; a phi that merges the
same value from both sides is replaced by it.

## Used by
Named by `pipeline`, `pipeline_text`.
