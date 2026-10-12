> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/twoaddr.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/twoaddr.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/twoaddr.rs sha256:98de92fa7b68bc771ea9a9d2d53996f9b6c83e6dc912e09e5ce0393b5809605c -->

# twoaddr

Port of `qbopt/backend/twoaddr.py`: x86 writes into one of the registers
it reads.

LLVM's `TwoAddressInstructionPass`: `c := a + b` becomes `c := a` then
`c := c + b`.
