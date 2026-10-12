> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/timing.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/timing.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/timing.rs sha256:5c9d279434d6c5359c3ae5f11d178f5da3461ff99042c5cef337590c541a203e -->

# timing

Port of `qbopt/backend/timing.py`: audited instruction core-clock
bounds, separate from scoreboard estimates.

Where it is used: docs/machine/arithmetic-targets.md. Bounds exclude
decode, prefix, memory and scheduling costs.
