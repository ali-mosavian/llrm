> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/effects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/effects.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/effects.rs sha256:6174d853e4cdb18ffe1679628f73c5f12bc602eb85f5c23233ddfcb04b12f854 -->

# effects

The registers and flags an instruction reads and writes, as `x86.instr`
states them: the rows the target's register file carries
(`Info::effects`), for the code of `bits` bits.
