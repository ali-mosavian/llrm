> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopshape.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopshape.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopshape.rs sha256:f0cf3dd6b40cc94544346b2b79f8bc59455107a5665c75c086b0a6cf518a9691 -->

# loopshape

What loop fusion and loop distribution ask of a loop alike: that one
counter's test is the loop's only exit, that its body only reads and
writes memory and cannot trap, and that nothing it computes is read
outside it.

## How it works
`controls(function, proof, shape)`: `proof` counts the loop from its one
exit, which is in the latch where the loop is tested after each trip and in
the header where it is tested before (and the header then not its own
latch). `quiet(..)`: every instruction of the body is a phi, a branch, a
load, a store, or work with no effect that cannot trap. `header_quiet(..)`:
the header of a loop tested before each trip runs once more than its body,
so its work must be such that running once more or not at all changes
nothing: no memory access, as far as a header's own work goes.
`read_outside(..)`: a value an instruction of the loop defines is read by
an instruction outside it.

## Example
None: a library. [`crate::versionalias`](versionalias.md) asks all four of its one loop.

## Provides
`controls`, `quiet`, `header_quiet`, `read_outside`.

## See also
LLVM `LoopFuse`'s `isEligibleForFusion`/`containsUnsafeInstructions` and
`LoopDistribute`'s refusal of values used outside the loop.
