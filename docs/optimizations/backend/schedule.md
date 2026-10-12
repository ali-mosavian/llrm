> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/schedule.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/schedule.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/schedule.rs sha256:685a448016c8d2d42d350d8cce22ad0592a69cb59e21455a7038f0820803bec6 -->

# schedule

Port of `qbopt/backend/schedule.py`: latency-aware ordering of safe,
already-allocated machine instructions.

This is deliberately narrower than an instruction scheduler in a flat
32-bit compiler.  The medium-model output has segment state, far calls,
source-map anchors, and precise x87 exception ordering.  Until all of
those have a complete dependency model, they are boundaries.  Within the
remaining integer register/immediate windows, physical register and flag
lanes are the complete dependency graph, so a later independent operation
can fill the latency of an earlier producer without changing memory or
ABI behaviour.
