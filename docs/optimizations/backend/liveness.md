> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/liveness.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/liveness.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/liveness.rs sha256:c23a0653ff8240c1e1696a2e13d773efaeae447a872fae158ee65f50ebedc596 -->

# liveness

Port of `qbopt/backend/liveness.py`: which physical register and flag
lanes are dead on exit from each block.

A backward walk inside one block starts by assuming everything is live, so a
copy written as the last instruction of a block always survives it -- and a
parallel copy for a phi is written exactly there. deedlines' plasmablobs
ends its inner loop with `mov di,bx` whose destination no path reads before
writing it again.
