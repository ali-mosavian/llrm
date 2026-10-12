> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/selects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/selects.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/selects.rs sha256:3adf37d43a5fe8340cf01454c4c378e1ac2e65f14d5d371cbf724742db830681 -->

# selects

Selects as branches: the 486 has no conditional move, so each `select
c, a, b` becomes a branch on `c` around an empty block into a phi of
`a` and `b`, before instruction selection. LLVM's CodeGenPrepare does
the same where the target prices a branch below a conditional move.
