> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/masm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/masm.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/masm.rs sha256:edf9275fd345378911eed3812899bc264ec20ad658abc9d51235960675245324 -->

# masm

Allocated LIR as jwasm source.

Port of `qbopt/backend/masm.py`. `listing` is the procedure as emitted,
frame and all; this prints it and objbuild.rs encodes it, so the two
cannot drift. Every operand is already placed; an unplaced one is an
error.
