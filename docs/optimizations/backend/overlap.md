> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/overlap.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/overlap.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/overlap.rs sha256:608a4badb6cfcb9b92c94fad84482ce7fa64d2f2dd64e5e1b8374946129844e2 -->

# overlap

Whether two machine memory operands may overlap, read from their
addresses alone: the same space and segment compared by offset and
width, and "may overlap" for anything else.

This stays syntactic. A pass that needs a semantic answer for memory
outside the frame (two globals, a pointer) gets it from the rich MIR,
carried on the instruction as `CallMemory` carries a call's effects,
never re-derived here.
