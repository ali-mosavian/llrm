> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/selects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/selects.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/selects.rs sha256:09e797a5a6e6d906d113d4e893f2472f6eff9fe2f0d17641379c1df428adf5cc -->

# selects

Selects as branches: the 486 has no conditional move, so each `select
c, a, b` becomes a branch on `c` around an empty block into a phi of
`a` and `b`, before instruction selection. LLVM's CodeGenPrepare does
the same where the target prices a branch below a conditional move.
