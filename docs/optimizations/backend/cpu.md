> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/cpu.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/cpu.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/cpu.rs sha256:ee360a2089105db095cfa85fb0c792a50dfe1c1c5ddc04c5c6d101b58f1328be -->

# cpu

Port of `qbopt/backend/cpu.py`: one immutable description of a
code-generation tuning target.

Costs from `cycles::timings` are rankings; `timing` holds the smaller
audited subset used for legality-changing decisions.
