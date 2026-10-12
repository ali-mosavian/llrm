> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/twoaddr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/twoaddr.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/twoaddr.rs sha256:be743936399d7460688793a3dc5fd4bc3893e345a5d326606b22f0de17b0980e -->

# twoaddr

Port of `qbopt/backend/twoaddr.py`: x86 writes into one of the registers
it reads.

LLVM's `TwoAddressInstructionPass`: `c := a + b` becomes `c := a` then
`c := c + b`.
