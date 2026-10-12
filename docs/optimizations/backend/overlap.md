> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/overlap.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/overlap.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/overlap.rs sha256:d24b4bf0712ba75e9f2cd813f8b4ef751fec0b8c30d6eb9e556d21b2acdd98e5 -->

# overlap

Whether two machine memory operands may overlap, read from their
addresses alone: the same space and segment compared by offset and
width, and "may overlap" for anything else.

This stays syntactic. A pass that needs a semantic answer for memory
outside the frame (two globals, a pointer) gets it from the rich MIR,
carried on the instruction as `CallMemory` carries a call's effects,
never re-derived here.
