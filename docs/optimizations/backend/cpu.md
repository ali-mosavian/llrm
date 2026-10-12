> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/cpu.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/cpu.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/cpu.rs sha256:dfc4f7b39659a2c4c147e600bcf39e92b1f952e3d21979fea20e6c93ec5ae924 -->

# cpu

Port of `qbopt/backend/cpu.py`: one immutable description of a
code-generation tuning target.

Costs from `cycles::timings` are rankings; `timing` holds the smaller
audited subset used for legality-changing decisions.
