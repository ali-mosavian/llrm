> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/liveness.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/liveness.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/liveness.rs sha256:f1a2e479cf0492b4823fef8c51867f51ec97a3082ca23137a6efb02dd3a3a594 -->

# liveness

Port of `qbopt/backend/liveness.py`: which physical register and flag
lanes are dead on exit from each block.

A backward walk inside one block starts by assuming everything is live, so a
copy written as the last instruction of a block always survives it -- and a
parallel copy for a phi is written exactly there. deedlines' plasmablobs
ends its inner loop with `mov di,bx` whose destination no path reads before
writing it again.
