> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/expand.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/expand.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/expand.rs sha256:50b30c611f9151e6e50cf2c3b14b319f28d33c52b22be8d21f69275709ee8405 -->

# expand

Values placed from their sums: LLVM's `SCEVExpander`. A `Scev` is
built of its terms scaled and added once each, before one instruction,
and the least of several as compares and selects.
