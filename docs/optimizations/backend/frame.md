> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/frame.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/frame.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/frame.rs sha256:dd06c08bf3bc55b5c4773e5bc6e59c15961699074c63cf12126734bf9dcde31c -->

# frame

Port of `qbopt/backend/frame.py`: the stack slots a body needs, and where
they are.

A slot is two bytes at `[bp-n]`, and `size` is what the prologue has to
take off sp.
