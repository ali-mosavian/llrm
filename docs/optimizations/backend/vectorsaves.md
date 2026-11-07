> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/vectorsaves.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/vectorsaves.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/vectorsaves.rs sha256:22b213f90851a869056974b74024195682f8cb30188df56327835d6ca1707a4f -->

# vectorsaves

The callee-saved vector registers a body writes get a 16-byte frame cell
each (X86FrameLowering::spillCalleeSavedRegisters: xmm is saved with
`movaps` into the callee-saved area, Win64's xmm6-15). Only the cells are
made here, after allocation and before the call area is reserved; the
entry and the exits save and reload them (`masm::lowered`).

## Example
A Win64 function that writes xmm6 and xmm7 has `vector_saves` of
two adjacent 16-byte cells below its spills.

## Used by
Named by `flow`.
