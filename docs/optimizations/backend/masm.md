> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/masm.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/masm.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/masm.rs sha256:087692eb09bc964756bb6490b209844ca254a356f9990a2863693274ed194162 -->

# masm

Allocated LIR as jwasm source.

Port of `qbopt/backend/masm.py`. `listing` is the procedure as emitted,
frame and all; this prints it and objbuild.rs encodes it, so the two
cannot drift. Every operand is already placed; an unplaced one is an
error.
