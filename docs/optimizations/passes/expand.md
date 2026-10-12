> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/expand.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/expand.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/expand.rs sha256:e1b3996280e912c60bec8348d110e7964c8e0c58f159556a337476c96c7e14a9 -->

# expand

Values placed from their sums: LLVM's `SCEVExpander`. A `Scev` is
built of its terms scaled and added once each, before one instruction,
and the least of several as compares and selects.
