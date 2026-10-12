> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/timing.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/timing.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/timing.rs sha256:a5fd30fe9c59a283dd1cc690ecba217dae0a0909365112f7ba20d09f6ab8182a -->

# timing

Port of `qbopt/backend/timing.py`: audited instruction core-clock
bounds, separate from scoreboard estimates.

Where it is used: docs/machine/arithmetic-targets.md. Bounds exclude
decode, prefix, memory and scheduling costs.
