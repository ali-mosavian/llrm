> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/effects.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/effects.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/effects.rs sha256:76573f6940ab3399ac52cb6561e37b4503771ac792575dd1eb05c097e03a86d1 -->

# effects

The registers and flags an instruction reads and writes, as `x86.instr`
states them: the rows the target's register file carries
(`Info::effects`), for the code of `bits` bits.
