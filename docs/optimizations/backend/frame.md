> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/frame.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/frame.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/frame.rs sha256:7b2c6f213f18fe36aa6aef5c26b90aa89b3d132c9c3c7f494321feda0623d8d9 -->

# frame

Port of `qbopt/backend/frame.py`: the stack slots a body needs, and where
they are.

A slot is two bytes at `[bp-n]`, and `size` is what the prologue has to
take off sp.
